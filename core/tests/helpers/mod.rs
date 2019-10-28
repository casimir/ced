use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use channel::Receiver;

use ced::editor::Editor;
use ced::remote::jsonrpc::Notification;
use ced::remote::protocol::notifications::ViewParams;
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
    pub view: ViewParams,
}

impl State {
    pub fn new(rx: Receiver<BroadcastMessage>) -> State {
        State {
            rx,
            view: ViewParams::default(),
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

pub fn assert_buffers(view: &ViewParams, buffers: Vec<String>) {
    let view_buffers: Vec<String> = view.iter().map(|item| item.buffer.to_owned()).collect();
    assert_eq!(buffers, view_buffers);
}
