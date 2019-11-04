use std::io::{self, Write};
use std::ops::Drop;
use std::thread;
use std::time::Duration;

use channel::select;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    input::{InputEvent, TerminalInput},
    queue,
    screen::AlternateScreen,
    style::{style, Colorize, PrintStyledContent, Styler},
    terminal::{self, Clear, ClearType},
    Output, Result as CTResult,
};
use remote::protocol::{Face, Key, KeyEvent, TextFragment};
use remote::{Connection, ConnectionEvent, Menu, Session};

enum Event {
    Input(InputEvent),
    Resize(u16, u16),
}

fn format_text(tf: &TextFragment) -> String {
    match tf.face {
        Face::Default => tf.text.to_owned(),
        Face::Error => style(&tf.text).red().to_string(),
        Face::Selection => style(&tf.text).reverse().to_string(),
        _ => tf.text.to_owned(),
    }
}

pub struct Term {
    connection: Connection,
    exit_pending: bool,
    last_size: (u16, u16),
    _screen: AlternateScreen,
}

impl Term {
    pub fn new(session: &Session, filenames: &[&str]) -> io::Result<Term> {
        let mut term = Term {
            connection: Connection::new(session)?,
            exit_pending: false,
            last_size: terminal::size().expect("get terminal"),
            _screen: AlternateScreen::to_alternate(true)
                .expect("enable raw mode and switch to alternate screen"),
        };
        execute!(io::stdout(), Hide).expect("hide cursor");
        for fname in filenames {
            term.connection.edit(fname, false);
        }
        Ok(term)
    }

    pub fn start(&mut self) {
        let (events_tx, events_rx) = channel::unbounded();
        let keys_tx = events_tx.clone();
        thread::spawn(move || {
            let mut input = TerminalInput::new().read_sync();
            // input().enable_mouse_mode().expect("enable mouse events");
            // TODO and_then?
            loop {
                if let Some(key) = input.next() {
                    keys_tx.send(Event::Input(key)).expect("send key event");
                }
            }
        });
        let resize_tx = events_tx.clone();
        let starting_size = self.last_size;
        thread::spawn(move || {
            let mut current = starting_size;
            loop {
                match terminal::size() {
                    Ok(size) => {
                        if current != size {
                            resize_tx
                                .send(Event::Resize(size.0, size.1))
                                .expect("send resize event");
                            current = size;
                        }
                    }
                    Err(e) => log::error!("{}", e),
                }
                thread::sleep(Duration::from_millis(20));
            }
        });

        let messages = self.connection.connect();
        while !self.exit_pending {
            use ConnectionEvent::*;
            select! {
                recv(messages) -> msg => match msg {
                    Ok(ev) => match ev {
                        Menu(menu) => self.draw_menu(&menu).expect("draw menu"),
                        Echo(_)|Status(_)|View(_) => self.draw_view().expect("draw view"),
                        Info(_, _) => {},
                    }
                    Err(_) => break,
                },
                recv(events_rx) -> event => match event {
                    Ok(Event::Input(input)) => self.handle_input(input).expect("handle input"),
                    Ok(Event::Resize(w, h)) => self.resize(w, h).expect("resize"),
                    Err(_) => break,
                }
            }
        }
    }

    fn draw_view(&mut self) -> CTResult<()> {
        let mut stdout = io::stdout();
        let (width, height) = self.last_size;
        let state = self.connection.state();

        queue!(stdout, Clear(ClearType::All))?;
        {
            let mut i = 0;
            let mut content = Vec::new();
            'outer: for item in &state.view {
                let buffer = &item.buffer;
                let coords = format!("{}:{}", item.start, item.end);
                let padding = "-".repeat(width as usize - 5 - buffer.len() - coords.len());
                content.push(format!("-[{}][{}]{}", buffer, coords, padding));
                i += 1;

                for lens in &item.lenses {
                    for line in &lens.lines {
                        if i == (height - 1) {
                            break 'outer;
                        }
                        let rendered = line.render(format_text);
                        let line_view = if line.text_len() > width as usize {
                            &rendered[..width as usize]
                        } else {
                            &rendered
                        };
                        content.push(line_view.to_string());
                        i += 1;
                    }
                }
            }
            queue!(stdout, MoveTo(0, 0), Output(content.join("\r\n")))?;
        }

        let echo = state.echo.unwrap_or_default();
        let status = state
            .status
            .iter()
            .map(|item| item.text.plain())
            .collect::<Vec<String>>()
            .join("·");
        let text = if echo.text_len() >= width as usize {
            echo.render(format_text)
        } else if echo.text_len() + status.len() >= width as usize {
            let skip = echo.text_len() - width as usize + 1;
            format!("{} {}", echo.render(format_text), &status[skip..])
        } else {
            let padding = width as usize - echo.text_len() - status.len();
            format!(
                "{}{}{}",
                echo.render(format_text),
                " ".repeat(padding),
                status
            )
        };
        queue!(
            stdout,
            MoveTo(0, height),
            PrintStyledContent(style(text).reverse())
        )?;
        stdout.flush()?;
        Ok(())
    }

    fn draw_menu(&mut self, menu: &Menu) -> CTResult<()> {
        let mut stdout = io::stdout();
        let (width, height) = self.last_size;
        let title = format!("{}:{}", menu.title, menu.search);
        let padding = " ".repeat(width as usize - title.len());

        queue!(
            stdout,
            Clear(ClearType::All),
            MoveTo(0, 0),
            PrintStyledContent(style(title + &padding).reverse())
        )?;
        {
            let display_size = (height - 1) as usize;
            for i in 0..menu.entries.len() {
                if i == display_size {
                    break;
                }
                let item = &menu.entries[i].text.render(|tf| match tf.face {
                    Face::Match => style(&tf.text).underlined().to_string(),
                    _ => tf.text.to_owned(),
                });

                let item_view = if item.len() > width as usize {
                    &item[..width as usize]
                } else {
                    &item
                };
                let item_print = if i == menu.selected {
                    PrintStyledContent(style(item_view).reverse())
                } else {
                    PrintStyledContent(style(item_view))
                };
                queue!(
                    stdout,
                    MoveTo(0, 1 + i as u16),
                    item_print,
                    Clear(ClearType::UntilNewLine)
                )?;
            }
        }
        stdout.flush()?;
        Ok(())
    }

    fn resize(&mut self, w: u16, h: u16) -> CTResult<()> {
        let current = (w, h);
        if self.last_size != current {
            self.last_size = current;
            match self.connection.state().menu {
                Some(menu) => self.draw_menu(&menu)?,
                None => self.draw_view()?,
            }
        }
        Ok(())
    }

    fn do_menu(&mut self, command: &str, search: &str) {
        self.connection.menu(command, search);
    }

    fn handle_input(&mut self, event: InputEvent) -> CTResult<()> {
        use crossterm::input::{InputEvent::*, KeyEvent::*};
        if let Some(menu) = self.connection.state().menu {
            match event {
                Keyboard(Esc) => {
                    self.connection.action_menu_cancel();
                    self.draw_view()?;
                }
                Keyboard(Enter) => {
                    self.connection.menu_select();
                    self.draw_view()?;
                }
                Keyboard(Up) => {
                    self.connection.action_menu_select_previous();
                    let new_menu = self.connection.state().menu.unwrap();
                    self.draw_menu(&new_menu)?;
                }
                Keyboard(Down) => {
                    self.connection.action_menu_select_next();
                    let new_menu = self.connection.state().menu.unwrap();
                    self.draw_menu(&new_menu)?;
                }
                Keyboard(Char(c)) => {
                    let mut search = menu.search;
                    search.push(c);
                    self.do_menu(&menu.command, &search)
                }
                Keyboard(Backspace) => {
                    let mut search = menu.search;
                    search.pop();
                    self.do_menu(&menu.command, &search)
                }
                _ => {}
            }
        } else {
            match event {
                Keyboard(Alt('q')) => self.exit_pending = true,
                Keyboard(Ctrl('f')) => self.do_menu("open", ""),
                Keyboard(Ctrl('p')) => self.do_menu("", ""),
                Keyboard(Ctrl('v')) => self.do_menu("view_select", ""),
                Keyboard(Ctrl('x')) => panic!("panic mode activated!"),
                Keyboard(Char(c)) => self.connection.keys(KeyEvent::from(c)),
                Keyboard(Ctrl(c)) => {
                    let mut key = KeyEvent::from(c);
                    key.ctrl = true;
                    self.connection.keys(key)
                }
                Keyboard(Alt(c)) => {
                    let mut key = KeyEvent::from(c);
                    key.alt = true;
                    self.connection.keys(key)
                }
                Keyboard(Esc) => self.connection.keys(KeyEvent::from(Key::Escape)),
                _ => {}
            }
        }
        Ok(())
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show)
            .map_err(|e| eprintln!("could not revert cursor state: {}", e));
    }
}
