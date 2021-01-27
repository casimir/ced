use std::io::{self, Write};

use crossterm::{
    cursor,
    event::{Event as CEvent, EventStream, KeyCode as CKeyCode, KeyModifiers as CKeyModifiers},
    execute, queue,
    style::{style, Colorize, Print, PrintStyledContent, Styler},
    terminal::{self, Clear, ClearType},
    Result as CTResult,
};
use futures_lite::*;
use remote::protocol::{notifications::HintParams, Face, Key, KeyEvent, TextFragment};
use remote::{Connection, ConnectionEvent, Menu, Session};
use smol::channel::bounded;

fn logline(msg: impl std::fmt::Display) {
    use std::fs::OpenOptions;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("log.txt")
        .expect("open file");

    if let Err(e) = writeln!(file, "{}", msg) {
        eprintln!("Couldn't write to file: {}", e);
    }
}

struct ToKey(Option<KeyEvent>);

impl From<CEvent> for ToKey {
    fn from(ev: CEvent) -> ToKey {
        if let CEvent::Key(ke) = ev {
            let key = match ke.code {
                CKeyCode::Char(c) => Some(Key::Char(c)),

                CKeyCode::Backspace => Some(Key::Backspace),
                CKeyCode::Enter => Some(Key::Enter),
                CKeyCode::Esc => Some(Key::Escape),

                CKeyCode::Up => Some(Key::Up),
                CKeyCode::Down => Some(Key::Down),
                CKeyCode::Left => Some(Key::Left),
                CKeyCode::Right => Some(Key::Right),

                _ => None,
            };
            ToKey(key.map(|k| KeyEvent {
                ctrl: ke.modifiers.contains(CKeyModifiers::CONTROL),
                alt: ke.modifiers.contains(CKeyModifiers::ALT),
                shift: ke.modifiers.contains(CKeyModifiers::SHIFT),
                key: k,
            }))
        } else {
            ToKey(None)
        }
    }
}

fn format_text(tf: &TextFragment) -> String {
    match tf.face {
        Face::Default => tf.text.to_owned(),
        Face::Error => style(&tf.text).red().to_string(),
        Face::Selection => style(&tf.text).reverse().to_string(),
        _ => tf.text.to_owned(),
    }
}

#[derive(Debug)]
enum Event {
    DrawHint(HintParams),
    DrawMenu(Menu),
    DrawStatus,
    DrawView,
    Error(String),
    Input(CEvent),
    Resize((u16, u16)),
}

pub struct Term {
    connection: Connection,
    exit_pending: bool,
    last_size: (u16, u16),
}

impl Term {
    pub fn new(session: Session, filenames: &[&str]) -> io::Result<Term> {
        logline("----------");
        let mut term = Term {
            connection: Connection::new(session)?,
            exit_pending: false,
            last_size: terminal::size().expect("get terminal"),
        };
        logline("new connection");
        for fname in filenames {
            term.connection.edit(fname.to_string(), false);
        }
        Ok(term)
    }

    pub fn start(&mut self) {
        terminal::enable_raw_mode().expect("enable terminal raw mode");
        execute!(io::stdout(), terminal::EnterAlternateScreen, cursor::Hide)
            .expect("prepare terminal state");

        smol::block_on(async {
            let (tx, rx) = bounded(100);

            let events_tx = tx.clone();
            smol::spawn(async move {
                let tx = events_tx;
                let mut reader = EventStream::new();
                while let Some(event) = reader.next().await {
                    logline(format!("new input: {:?}", event));
                    match event {
                        Ok(ev @ CEvent::Key(_)) => {
                            tx.send(Event::Input(ev)).await.expect("send event")
                        }
                        Ok(CEvent::Mouse(_)) => {}
                        Ok(CEvent::Resize(w, h)) => {
                            tx.send(Event::Resize((w, h))).await.expect("send event")
                        }
                        Err(e) => tx
                            .send(Event::Error(format!("\"event read: {}\"", e)))
                            .await
                            .expect("send event"),
                    }
                }
            })
            .detach();

            let (mut messages, request_loop) = self.connection.connect().await;
            smol::spawn(request_loop).detach();
            let messages_tx = tx.clone();
            smol::spawn(async move {
                let tx = messages_tx;
                while let Some(msg) = messages.next().await {
                    logline(format!("new message: {:?}", msg));
                    use ConnectionEvent::*;
                    match msg {
                        Echo(_) | View(_) => tx.send(Event::DrawView).await.expect("send event"),
                        Hint(hint) => tx.send(Event::DrawHint(hint)).await.expect("send event"),
                        Info(_, _) => {}
                        Menu(menu) => tx.send(Event::DrawMenu(menu)).await.expect("send event"),
                        Status(_) => tx.send(Event::DrawStatus).await.expect("send event"),
                        ConnErr(msg) => tx.send(Event::Error(msg)).await.expect("send event"),
                        Noop => {}
                    }
                }
                logline("connection lost");
                tx.close();
            })
            .detach();

            while let Ok(event) = rx.recv().await {
                logline(format!("new event: {:?}", event));
                use Event::*;
                match event {
                    DrawMenu(menu) => self.draw_menu(&menu).expect("draw menu"),
                    DrawView => self.draw_view().expect("draw view"),
                    DrawHint(h) => self.draw_hint(&h).expect("draw hint"),
                    DrawStatus => self.draw_status(true).expect("draw view"),
                    Error(msg) => self.error(&msg),
                    Input(ev) => self.handle_input(ev).expect("handle input"),
                    Resize((w, h)) => self.resize(w, h).expect("resize"),
                }
                if self.exit_pending {
                    break;
                }
            }
        });
    }

    fn debug(&mut self, message: &str) {
        self.connection.exec(&format!("editor:debug({})", message));
    }

    fn error(&mut self, message: &str) {
        self.connection.exec(&format!("editor:error({})", message));
    }

    fn draw_view(&mut self) -> CTResult<()> {
        let mut stdout = io::stdout();
        let (width, height) = self.last_size;
        let state = self.connection.state();

        queue!(stdout, Clear(ClearType::All))?;
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
        queue!(stdout, cursor::MoveTo(0, 0), Print(content.join("\r\n")))?;

        self.draw_status(false)?; // TODO don't ClearAll and don't redraw each time
        stdout.flush()?;
        Ok(())
    }

    fn draw_hint(&mut self, hint: &HintParams) -> CTResult<()> {
        let mut stdout = io::stdout();
        let (width_w, height_w) = self.last_size;

        let width_h = hint.text[0].plain().chars().count();
        let x = width_w - width_h as u16;
        let mut y = height_w - hint.text.len() as u16;
        for line in &hint.text {
            // TODO clamp to screen size (or less?)
            let text = line.render(format_text);
            queue!(
                stdout,
                cursor::MoveTo(x, y),
                PrintStyledContent(style(text).reverse())
            )?;
            y += 1;
            if y == height_w {
                break;
            }
        }

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
            cursor::MoveTo(0, 0),
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
                    cursor::MoveTo(0, 1 + i as u16),
                    item_print,
                    Clear(ClearType::UntilNewLine)
                )?;
            }
        }
        stdout.flush()?;
        Ok(())
    }

    fn draw_status(&mut self, flush: bool) -> CTResult<()> {
        let mut stdout = io::stdout();
        let (width, height) = self.last_size;
        let state = self.connection.state();

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
            cursor::MoveTo(0, height),
            PrintStyledContent(style(text).reverse())
        )?;

        if flush {
            stdout.flush()?;
        }
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

    fn handle_input(&mut self, value: impl Into<ToKey>) -> CTResult<()> {
        let key = match value.into().0 {
            Some(k) => k,
            None => return Ok(()),
        };
        if let Some(menu) = self.connection.state().menu {
            match key {
                KeyEvent {
                    key: Key::Escape, ..
                } => {
                    self.connection.action_menu_cancel();
                    self.draw_view()?;
                }
                KeyEvent {
                    key: Key::Enter, ..
                } => {
                    self.connection.menu_select();
                    self.draw_view()?;
                }
                KeyEvent { key: Key::Up, .. } => {
                    self.connection.action_menu_select_previous();
                    let new_menu = self.connection.state().menu.unwrap();
                    self.draw_menu(&new_menu)?;
                }
                KeyEvent { key: Key::Down, .. } => {
                    self.connection.action_menu_select_next();
                    let new_menu = self.connection.state().menu.unwrap();
                    self.draw_menu(&new_menu)?;
                }
                KeyEvent {
                    key: Key::Char(c), ..
                } => {
                    let mut search = menu.search;
                    search.push(c);
                    self.do_menu(&menu.command, &search)
                }
                KeyEvent {
                    key: Key::Backspace,
                    ..
                } => {
                    let mut search = menu.search;
                    search.pop();
                    self.do_menu(&menu.command, &search)
                }
                _ => {}
            }
        } else {
            match key {
                KeyEvent {
                    key: Key::Char('q'),
                    alt: true,
                    ..
                } => self.exit_pending = true,
                KeyEvent {
                    key: Key::Char('d'),
                    ctrl: true,
                    ..
                } => self.connection.exec("editor:debug('coucou')"),
                KeyEvent {
                    key: Key::Char('e'),
                    ctrl: true,
                    ..
                } => self.connection.exec("editor:debug(undefined)"),
                KeyEvent {
                    key: Key::Char('f'),
                    ctrl: true,
                    ..
                } => self.do_menu("open", ""),
                KeyEvent {
                    key: Key::Char('p'),
                    ctrl: true,
                    ..
                } => self.do_menu("", ""),
                KeyEvent {
                    key: Key::Char('v'),
                    ctrl: true,
                    ..
                } => self.do_menu("view_select", ""),
                KeyEvent {
                    key: Key::Char('x'),
                    ctrl: true,
                    ..
                } => panic!("panic mode activated!"),
                k @ KeyEvent {
                    key: Key::Char(_), ..
                } => self.connection.keys(k),
                k @ KeyEvent {
                    key: Key::Escape, ..
                } => self.connection.keys(k),
                _ => {}
            }
        }
        Ok(())
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        terminal::disable_raw_mode().expect("disable terminal raw mode");
        let _ = execute!(io::stdout(), terminal::LeaveAlternateScreen, cursor::Show)
            .map_err(|e| eprintln!("revert terminal state: {}", e));
    }
}
