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

use remote::protocol::{notifications, Face};
use remote::{Connection, ConnectionEvent, Menu, Session};

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
        for fname in filenames {
            term.connection.edit(fname, true);
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
                    Ok(ev) => match ev {
                        ConnectionEvent::Info(_, _)|ConnectionEvent::View(_) => self.draw_view(),
                        ConnectionEvent::Menu(menu) => self.draw_menu(&menu),
                    }
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

    fn draw_view(&mut self) {
        let (width, height) = self.last_size;
        write!(self.screen, "{}", termion::clear::All,).unwrap();

        let state = self.connection.state();
        {
            let mut i = 0;
            let mut content = Vec::new();
            'outer: for item in &state.view {
                use notifications::ViewParamsItem::*;
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

        let client_label = format!("[{}@{}]", state.client, state.session);
        let padding = " ".repeat(width as usize - client_label.len());
        write!(
            self.screen,
            "{}{}{}{}{}",
            Goto(1, height),
            termion::style::Invert,
            padding,
            client_label,
            termion::style::Reset
        )
        .unwrap();
        self.flush();
    }

    fn draw_menu(&mut self, menu: &Menu) {
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
        self.flush();
    }

    fn resize(&mut self, w: u16, h: u16) {
        let current = (w, h);
        if self.last_size != current {
            self.last_size = current;
            match self.connection.state().menu {
                Some(menu) => self.draw_menu(&menu),
                None => self.draw_view(),
            }
        }
    }

    fn do_menu(&mut self, command: &str, search: &str) {
        self.connection.menu(command, search);
    }

    fn handle_key(&mut self, key: Key) {
        if let Some(menu) = self.connection.state().menu {
            match key {
                Key::Esc => {
                    self.connection.action_menu_cancel();
                    self.draw_view();
                }
                Key::Char('\n') => {
                    self.connection.menu_select();
                    self.draw_view();
                }
                Key::Up => {
                    self.connection.action_menu_select_previous();
                    let new_menu = self.connection.state().menu.unwrap();
                    self.draw_menu(&new_menu);
                }
                Key::Down => {
                    self.connection.action_menu_select_next();
                    let new_menu = self.connection.state().menu.unwrap();
                    self.draw_menu(&new_menu);
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
                _ => {}
            }
        } else {
            match key {
                Key::Esc => self.exit_pending = true,
                Key::Ctrl('f') => self.do_menu("open", ""),
                Key::Ctrl('p') => self.do_menu("", ""),
                Key::Ctrl('v') => self.do_menu("view_select", ""),
                Key::Ctrl('x') => panic!("panic mode activated!"),
                Key::Char(c) => self.connection.keys(vec![c.into()]),
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
