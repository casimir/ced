use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use crossbeam_channel::Receiver;

use ced::editor::Editor;
use ced::remote::jsonrpc::Notification;
use ced::remote::protocol::notification::view::{Params as View, ParamsItem as ViewItem};
use ced::server::{BroadcastMessage, Broadcaster};

pub fn root() -> PathBuf {
    let mut root = std::env::current_exe().unwrap();
    root.pop(); // bin
    root.pop(); // deps/
    root.pop(); // debug/
    root.pop(); // target/
    root
}

pub struct State {
    rx: Receiver<BroadcastMessage>,
    pub view: View,
}

impl State {
    pub fn new(rx: Receiver<BroadcastMessage>) -> State {
        State {
            rx,
            view: View::default(),
        }
    }

    fn update(&mut self, message: &Notification) {
        match message.method.as_str() {
            "view" => self.view = message.params().unwrap().unwrap(),
            _ => {}
        }
    }

    pub fn step(&mut self) -> usize {
        if let Ok(bm) = self.rx.recv() {
            self.update(&bm.message);
        }
        let mut count = 1;
        while let Ok(bm) = self.rx.try_recv() {
            self.update(&bm.message);
            count += 1;
        }
        count
    }
}

pub struct SequentialEditor {
    editor: Editor,
    state: State,
}

impl SequentialEditor {
    pub fn new() -> SequentialEditor {
        let broadcaster = Broadcaster::default();
        SequentialEditor {
            editor: Editor::new("", broadcaster.tx),
            state: State::new(broadcaster.rx),
        }
    }

    pub fn step(&mut self) -> usize {
        self.state.step()
    }

    pub fn state(&self) -> &State {
        &self.state
    }
}

impl Deref for SequentialEditor {
    type Target = Editor;

    fn deref(&self) -> &Editor {
        &self.editor
    }
}

impl DerefMut for SequentialEditor {
    fn deref_mut(&mut self) -> &mut Editor {
        &mut self.editor
    }
}

pub fn assert_buffers(view: &View, buffers: Vec<String>) {
    let view_buffers: Vec<String> = view
        .iter()
        .filter_map(|item| match item {
            ViewItem::Header(header) => Some(header.buffer.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(buffers, view_buffers);
}
