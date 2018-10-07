#![cfg(unix)]

use remote::jsonrpc::Request;
use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::ops::Drop;
use std::thread;
use std::time::Duration;

use crossbeam_channel as channel;
use failure::Error;
use ignore::Walk;
use termion;
use termion::cursor::Goto;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::screen::AlternateScreen;

use remote::jsonrpc::{ClientEvent, Id};
use remote::protocol;
use remote::protocol::notification::view::ParamsItem as ViewParamsItem;
use remote::{Client, Session};
use tui::finder::{Candidates, Finder};

struct Connection {
    client: Client,
    requests: channel::Sender<Request>,
    next_request_id: i32,
    pending: HashMap<Id, Request>,
}

impl Connection {
    fn new(session: &Session) -> Result<Connection, Error> {
        let (client, requests) = Client::new(session)?;
        Ok(Connection {
            client,
            requests,
            next_request_id: 0,
            pending: HashMap::new(),
        })
    }

    fn connect(&self) -> channel::Receiver<ClientEvent> {
        let messages_iter = self.client.run();
        let (tx, rx) = channel::unbounded();
        thread::spawn(move || {
            for msg in messages_iter {
                match msg {
                    Ok(m) => tx.send(m),
                    Err(e) => error!("{}", e),
                }
            }
        });
        rx
    }

    fn request_id(&mut self) -> Id {
        let id = self.next_request_id;
        self.next_request_id += 1;
        Id::Number(id)
    }

    fn request(&mut self, message: Request) {
        self.pending.insert(message.id.clone(), message.clone());
        self.requests.send(message);
    }
}

type Buffer = HashMap<String, String>;

struct Context {
    session: String,
    buffer_list: HashMap<String, Buffer>,
    buffer_current: String,
    view: Vec<ViewParamsItem>,
}

enum Event {
    Input(Key),
    Resize(u16, u16),
}

enum MenuChoice {
    Buffer(String),
    File(String),
    None,
}

struct Menu<'a> {
    title: &'a str,
    items: Vec<String>,
    search: String,
    candidates: Candidates,
    selected: usize,
    needs_redraw: bool,
    done: bool,
}

impl<'a> Menu<'a> {
    fn new(title: &'a str, items: Vec<String>) -> Menu<'a> {
        let mut menu = Menu {
            title,
            items,
            search: String::new(),
            candidates: Candidates::new(),
            selected: 0,
            needs_redraw: true,
            done: false,
        };
        menu.perform_search();
        menu
    }

    fn perform_search(&mut self) {
        let mut f = Finder::new(&self.search);
        self.candidates = f.search(&self.items);
        self.selected = 0;
        self.needs_redraw = true;
    }

    fn select_next(&mut self) {
        if self.selected < self.items.len() - 1 {
            self.selected += 1;
            self.needs_redraw = true;
        }
    }

    fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.needs_redraw = true;
        }
    }

    fn selected_item(&self) -> &str {
        &self.candidates[self.selected].text
    }

    fn choose(&mut self) -> MenuChoice {
        self.done = true;
        let choice = self.selected_item().to_owned();
        match self.title {
            "buffer" => MenuChoice::Buffer(choice),
            "file" => MenuChoice::File(choice),
            _ => unreachable!(),
        }
    }

    fn handle_key(&mut self, key: Key) -> MenuChoice {
        match key {
            Key::Esc => self.done = true,
            Key::Down => self.select_next(),
            Key::Up => self.select_previous(),
            Key::Char('\n') => {
                return self.choose();
            }
            Key::Char(c) => {
                self.search.push(c);
                self.perform_search();
            }
            Key::Backspace => {
                self.search.pop();
                self.perform_search();
            }
            _ => {}
        }
        MenuChoice::None
    }
}

macro_rules! process_params {
    ($msg:ident, $call:expr) => {
        match $msg.params() {
            Ok(Some(params)) => $call(params),
            Ok(None) => error!("missing field: params"),
            Err(err) => error!("{}", err),
        }
    };
}

macro_rules! process_result {
    ($msg:ident, $call:expr) => {
        match $msg.result() {
            Ok(Ok(result)) => $call(result),
            Ok(Ok(err)) => error!("error from server: {}"),
            Err(err) => error!("{}", err),
        }
    };
}

struct Term<'a> {
    connection: Connection,
    context: Context,
    exit_pending: bool,
    last_size: (u16, u16),
    screen: AlternateScreen<RawTerminal<io::Stdout>>,
    menu: Option<Menu<'a>>,
}

impl<'a> Term<'a> {
    fn new(session: &Session, filenames: &[&str]) -> Result<Term<'a>, Error> {
        let mut term = Term {
            connection: Connection::new(session)?,
            context: Context {
                session: String::new(),
                buffer_list: HashMap::new(),
                buffer_current: String::new(),
                view: Vec::new(),
            },
            exit_pending: false,
            screen: AlternateScreen::from(io::stdout().into_raw_mode()?),
            last_size: termion::terminal_size()?,
            menu: None,
        };
        term.cursor_visible(false);
        for filename in filenames {
            term.do_edit(filename);
        }
        Ok(term)
    }

    pub fn start(&mut self) -> Result<(), Error> {
        let (events_tx, events_rx) = channel::unbounded();
        let keys_tx = events_tx.clone();
        thread::spawn(move || {
            for key in io::stdin().keys() {
                match key {
                    Ok(k) => keys_tx.send(Event::Input(k)),
                    Err(e) => error!("{}", e),
                }
            }
        });
        let resize_tx = events_tx.clone();
        let starting_size = self.last_size;
        thread::spawn(move || {
            let mut current = starting_size;
            loop {
                match termion::terminal_size() {
                    Ok(size) => {
                        if current != size {
                            resize_tx.send(Event::Resize(size.0, size.1));
                            current = size;
                        }
                    }
                    Err(e) => error!("{}", e),
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let messages = self.connection.connect();
        while !self.exit_pending {
            select!{
                recv(messages, msg) => match msg {
                    Some(m) => self.handle_client_event(m),
                    None => break,
                }
                recv(events_rx, event) => match event {
                    Some(Event::Input(key)) => self.handle_key(key),
                    Some(Event::Resize(w, h)) => self.resize(w, h),
                    None => break,
                },
            }
        }
        Ok(())
    }

    fn flush(&mut self) {
        self.screen.flush().unwrap();
    }

    fn cursor_visible(&mut self, visible: bool) {
        if visible {
            write!(self.screen, "{}", termion::cursor::Show).unwrap();
        } else {
            write!(self.screen, "{}", termion::cursor::Hide).unwrap();
        };
        self.flush();
    }

    fn draw_buffer(&mut self) {
        let (width, height) = self.last_size;
        write!(self.screen, "{}", termion::clear::All,).unwrap();

        {
            let mut i = 0;
            let mut content = Vec::new();
            'outer: for item in &self.context.view {
                use self::ViewParamsItem::*;
                match item {
                    Header(header) => {
                        let buffer = &header.buffer;
                        let coords = format!("{}:{}", header.start, header.end);
                        let padding = "-".repeat(width as usize - 5 - buffer.len() - coords.len());
                        content.push(format!("-[{}][{}]{}", buffer, coords, padding));
                        i += 1;
                    }
                    Lines(lines) => {
                        for line in &lines.lines {
                            if i == (height - 1) {
                                break 'outer;
                            }
                            let line_view = if line.len() > width as usize {
                                &line[..width as usize]
                            } else {
                                &line
                            };
                            content.push(line_view.to_string());
                            i += 1;
                        }
                    }
                }
            }
            write!(self.screen, "{}{}", Goto(1, 1), content.join("\r\n")).unwrap();
        }

        let padding = " ".repeat(width as usize - 2 - self.context.session.len());
        write!(
            self.screen,
            "{}{}{}[{}]{}",
            Goto(1, height),
            termion::style::Invert,
            padding,
            self.context.session,
            termion::style::Reset
        ).unwrap();
    }

    fn draw_menu(&mut self) {
        if let Some(ref mut menu) = self.menu {
            if menu.needs_redraw {
                let (width, height) = self.last_size;
                write!(self.screen, "{}", termion::clear::All).unwrap();

                let title = format!("{}:{}", menu.title, menu.search);
                let padding = " ".repeat(width as usize - title.len());
                write!(
                    self.screen,
                    "{}{}{}{}{}",
                    Goto(1, 1),
                    termion::style::Invert,
                    title,
                    padding,
                    termion::style::Reset
                ).unwrap();

                {
                    let display_size = (height - 1) as usize;
                    for i in 0..menu.candidates.len() {
                        if i == display_size {
                            break;
                        }
                        let candidate = &menu.candidates[i];
                        if menu.candidates.has_matches() && !candidate.is_match() {
                            break;
                        }
                        let item = candidate.decorate(&|cap| {
                            format!(
                                "{}{}{}",
                                termion::style::Underline,
                                cap,
                                termion::style::NoUnderline,
                            )
                        });
                        let item_view = if item.len() > width as usize {
                            &item[..width as usize]
                        } else {
                            &item
                        };
                        if i == menu.selected {
                            write!(
                                self.screen,
                                "{}{}{}{}{}",
                                Goto(1, 2 + i as u16),
                                termion::style::Invert,
                                item_view,
                                termion::style::Reset,
                                termion::clear::UntilNewline
                            ).unwrap();
                        } else {
                            write!(
                                self.screen,
                                "{}{}{}",
                                Goto(1, 2 + i as u16),
                                item_view,
                                termion::clear::UntilNewline
                            ).unwrap();
                        }
                    }
                }
                menu.needs_redraw = false;
            }
        }
    }

    fn draw(&mut self) {
        if self.menu.is_some() {
            self.draw_menu();
        } else {
            self.draw_buffer();
        }
        self.flush();
    }

    fn resize(&mut self, w: u16, h: u16) {
        let current = (w, h);
        if self.last_size != current {
            self.last_size = current;
            self.draw();
        }
    }

    fn handle_client_event(&mut self, message: ClientEvent) {
        use self::ClientEvent::*;
        match message {
            Notification(notif) => match notif.method.as_str() {
                "info" => process_params!(notif, |params| self.process_info(params)),
                "view" => process_params!(notif, |params| self.process_view(params)),
                method => error!("unknown notification method: {}", method),
            },
            Response(resp) => match self.connection.pending.remove(&resp.id) {
                Some(req) => match req.method.as_str() {
                    "edit" | "buffer-select" => {}
                    method => error!("unknown response method: {}", method),
                },
                None => error!("unexpected response: {}", resp),
            },
        }
    }

    fn process_info(&mut self, params: protocol::notification::info::Params) {
        self.context.session = params.session;
        self.draw();
    }

    fn process_view(&mut self, params: protocol::notification::view::Params) {
        self.context.view = params;
        self.draw();
    }

    fn do_buffer_select(&mut self, buffer_name: &str) {
        let message =
            protocol::request::buffer_select::new(self.connection.request_id(), buffer_name);
        self.connection.request(message);
    }

    fn do_edit(&mut self, file_name: &str) {
        let message = protocol::request::edit::new(self.connection.request_id(), file_name);
        self.connection.request(message);
    }

    fn handle_key(&mut self, key: Key) {
        let mut needs_redraw = false;
        let mut menu_choice = MenuChoice::None;
        let mut remove_menu = false;
        if let Some(ref mut menu) = self.menu {
            menu_choice = menu.handle_key(key);
            needs_redraw = menu.needs_redraw;
            remove_menu = menu.done;
        } else {
            match key {
                Key::Esc => self.exit_pending = true,
                Key::Char('b') => {
                    let items: Vec<String> = self.context.buffer_list.keys().cloned().collect();
                    self.menu = Some(Menu::new("buffer", items));
                    self.draw();
                }
                Key::Char('e') => {
                    let current = &self.context.buffer_current.clone();
                    self.do_edit(current);
                }
                Key::Char('f') => {
                    let files: Vec<String> = Walk::new("./")
                        .into_iter()
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_type().map(|ft| !ft.is_dir()).unwrap_or(false))
                        .filter_map(|e| e.path().to_str().map(|s| String::from(&s[2..])))
                        .collect();
                    self.menu = Some(Menu::new("file", files));
                    self.draw();
                }
                Key::Char('p') => panic!("panic mode activated!"),
                _ => {}
            }
        }
        match menu_choice {
            MenuChoice::Buffer(name) => self.do_buffer_select(&name),
            MenuChoice::File(path) => {
                let mut absolute_path = env::current_dir().unwrap();
                absolute_path.push(path);
                self.do_edit(&absolute_path.to_str().unwrap());
            }
            MenuChoice::None => {}
        }
        if remove_menu {
            self.menu = None;
            needs_redraw = true;
        }
        if needs_redraw {
            self.draw();
        }
    }
}

impl<'a> Drop for Term<'a> {
    fn drop(&mut self) {
        self.cursor_visible(true);
    }
}

pub fn start(session: &Session, filenames: &[&str]) -> Result<(), Error> {
    Term::new(session, filenames)?.start()
}
