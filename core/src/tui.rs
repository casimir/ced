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
use remote::protocol::notification::view::ParamsItem as ViewParamsItem;
use remote::protocol::Face;
use remote::{Connection, Session};

enum Event {
    Input(Key),
    Resize(u16, u16),
}

pub struct Term {
    connection: Connection,
    exit_pending: bool,
    last_size: (u16, u16),
    screen: AlternateScreen<RawTerminal<io::Stdout>>,
}

impl Term {
    pub fn new(session: &Session, filenames: &[&str]) -> Result<Term, Error> {
        let mut term = Term {
            connection: Connection::new(session)?,
            exit_pending: false,
            screen: AlternateScreen::from(io::stdout().into_raw_mode()?),
            last_size: termion::terminal_size()?,
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
            select! {
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

        let state = self.connection.state();
        {
            let mut i = 0;
            let mut content = Vec::new();
            'outer: for item in &state.view {
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

        let padding = " ".repeat(width as usize - 2 - state.session.len());
        write!(
            self.screen,
            "{}{}{}[{}]{}",
            Goto(1, height),
            termion::style::Invert,
            padding,
            state.session,
            termion::style::Reset
        )
        .unwrap();
    }

    fn draw_menu(&mut self) {
        let menu = self.connection.state().menu.unwrap();
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
        )
        .unwrap();

        {
            let display_size = (height - 1) as usize;
            for i in 0..menu.entries.len() {
                if i == display_size {
                    break;
                }
                let item = &menu.entries[i]
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
                    })
                    .collect::<Vec<String>>()
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
                    )
                    .unwrap();
                } else {
                    write!(
                        self.screen,
                        "{}{}{}",
                        Goto(1, 2 + i as u16),
                        item_view,
                        termion::clear::UntilNewline
                    )
                    .unwrap();
                }
            }
        }
    }

    fn draw(&mut self) {
        if self.connection.state().menu.is_some() {
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
                "info" | "menu" | "view" => self.draw(),
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

    fn do_edit(&mut self, fname: &str) {
        self.connection.edit(fname);
    }

    fn do_menu(&mut self, command: &str, search: &str) {
        self.connection.menu(command, search);
    }

    fn handle_key(&mut self, key: Key) {
        if let Some(menu) = self.connection.state().menu {
            match key {
                Key::Esc => {
                    self.connection.action_menu_cancel();
                    self.draw();
                }
                Key::Up => {
                    self.connection.action_menu_select_previous();
                    self.draw();
                }
                Key::Down => {
                    self.connection.action_menu_select_next();
                    self.draw();
                }
                Key::Char('\n') => {
                    self.connection.menu_select();
                    self.draw();
                }
                Key::Char(c) => {
                    let mut search = menu.search;
                    search.push(c);
                    self.do_menu(&menu.command, &search)
                }
                Key::Backspace => {
                    let mut search = menu.search;
                    search.pop();
                    self.do_menu(&menu.command, &search)
                }
                _ => self.draw(),
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