#![cfg(all(feature = "term", unix))]

use std::io::{self, Write};
use std::ops::Drop;
use std::thread;
use std::time::Duration;

use crossbeam_channel as channel;
use failure::Error;
use termion;
use termion::cursor::Goto;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::screen::AlternateScreen;

use remote::jsonrpc::ClientEvent;
use remote::protocol::notification::{
    menu::Entry as MenuEntry, view::ParamsItem as ViewParamsItem,
};
use remote::protocol::{self, Face};
use remote::{Connection, Session};

enum Event {
    Input(Key),
    Resize(u16, u16),
}

struct Menu {
    command: String,
    title: String,
    items: Vec<MenuEntry>,
    search: String,
    selected: usize,
    needs_redraw: bool,
}

enum MenuAction {
    Choice,
    Cancel,
    Search,
    Noop,
}

impl Menu {
    fn new(command: String, title: String, search: String, items: Vec<MenuEntry>) -> Menu {
        Menu {
            command,
            title,
            items,
            search,
            selected: 0,
            needs_redraw: true,
        }
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
        &self.items[self.selected].value
    }

    fn handle_key(&mut self, key: Key) -> MenuAction {
        match key {
            Key::Esc => MenuAction::Cancel,
            Key::Down => {
                self.select_next();
                MenuAction::Noop
            }
            Key::Up => {
                self.select_previous();
                MenuAction::Noop
            }
            Key::Char('\n') => MenuAction::Choice,
            Key::Char(c) => {
                self.search.push(c);
                MenuAction::Search
            }
            Key::Backspace => {
                self.search.pop();
                MenuAction::Search
            }
            _ => MenuAction::Noop,
        }
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
            Ok(Err(err)) => error!("jsonrpc error: {}", err),
            Err(err) => error!("{}", err),
        }
    };
}

pub struct Term {
    connection: Connection,
    exit_pending: bool,
    last_size: (u16, u16),
    screen: AlternateScreen<RawTerminal<io::Stdout>>,
    menu: Option<Menu>,
}

impl Term {
    pub fn new(session: &Session, filenames: &[&str]) -> Result<Term, Error> {
        let mut term = Term {
            connection: Connection::new(session)?,
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
                    Ok(k) => keys_tx.send(Event::Input(k)).expect("send key event"),
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
                            resize_tx
                                .send(Event::Resize(size.0, size.1))
                                .expect("send resize event");
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
                recv(messages) -> msg => match msg {
                    Ok(m) => self.handle_client_event(m),
                    Err(_) => break,
                },
                recv(events_rx) -> event => match event {
                    Ok(Event::Input(key)) => self.handle_key(key),
                    Ok(Event::Resize(w, h)) => self.resize(w, h),
                    Err(_) => break,
                }
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

        let context = self.connection.context();
        {
            let mut i = 0;
            let mut content = Vec::new();
            'outer: for item in &context.view {
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

        let padding = " ".repeat(width as usize - 2 - context.session.len());
        write!(
            self.screen,
            "{}{}{}[{}]{}",
            Goto(1, height),
            termion::style::Invert,
            padding,
            context.session,
            termion::style::Reset
        ).unwrap();
    }

    fn draw_menu(&mut self) {
        let mut menu = self.menu.take().unwrap();
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
                for i in 0..menu.items.len() {
                    if i == display_size {
                        break;
                    }
                    let item = &menu.items[i]
                        .fragments
                        .iter()
                        .map(|f| match f.face {
                            Face::Match => format!(
                                "{}{}{}",
                                termion::style::Underline,
                                f.text,
                                termion::style::NoUnderline,
                            ),
                            _ => f.text.clone(),
                        }).collect::<Vec<String>>()
                        .join("");
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
            self.menu = Some(menu);
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
                "info" | "view" => self.draw(),
                "menu" => process_params!(notif, |params| self.process_menu(params)),
                method => error!("unknown notification method: {}", method),
            },
            Response(resp) => match self.connection.pending.remove(&resp.id) {
                Some(req) => match req.method.as_str() {
                    "edit" | "menu" | "menu-select" => {}
                    method => error!("unknown response method: {}", method),
                },
                None => error!("unexpected response: {}", resp),
            },
        }
    }

    fn process_menu(&mut self, result: protocol::notification::menu::Params) {
        self.menu = Some(Menu::new(
            result.command.to_owned(),
            result.title.to_owned(),
            result.search,
            result.entries,
        ));
        self.draw();
    }

    fn do_edit(&mut self, fname: &str) {
        let message = protocol::request::edit::new(self.connection.request_id(), fname);
        self.connection.request(message);
    }

    fn do_menu(&mut self, command: &str, search: &str) {
        let message = protocol::request::menu::new(self.connection.request_id(), command, search);
        self.connection.request(message);
    }

    fn do_menu_select(&mut self, command: &str, choice: &str) {
        let message =
            protocol::request::menu_select::new(self.connection.request_id(), command, choice);
        self.connection.request(message);
    }

    fn handle_key(&mut self, key: Key) {
        if self.menu.is_some() {
            let mut menu = self.menu.take().unwrap();
            match menu.handle_key(key) {
                MenuAction::Choice => {
                    self.do_menu_select(&menu.command, menu.selected_item());
                    self.menu = None;
                    self.draw();
                }
                MenuAction::Cancel => {
                    self.menu = None;
                    self.draw();
                }
                MenuAction::Search => {
                    self.do_menu(&menu.command, &menu.search);
                    self.menu = Some(menu);
                }
                MenuAction::Noop => {
                    self.menu = Some(menu);
                    self.draw();
                }
            }
        } else {
            match key {
                Key::Esc => self.exit_pending = true,
                Key::Char('f') => {
                    self.do_menu("open", "");
                }
                Key::Char('p') => {
                    self.do_menu("", "");
                }
                Key::Char('x') => panic!("panic mode activated!"),
                _ => {}
            }
        }
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        self.flush();
        self.cursor_visible(true);
    }
}
