use std::io::{self, BufRead};
use std::thread;

use crate::editor::Editor;
use crate::server::Broadcaster;
use remote::protocol::requests::EditParams;

const CLIENT_ID: usize = 1;

pub fn start_standalone(filenames: &[&str]) {
    let broadcaster = Broadcaster::default();
    let mut editor = Editor::new("", broadcaster.tx);

    editor.add_client(CLIENT_ID);
    for fname in filenames {
        let params = EditParams {
            file: fname.to_string(),
            path: None,
            scratch: false,
        };
        let _ = editor
            .command_edit(CLIENT_ID, &params)
            .map_err(|err| log::error!("could not open file '{}': {}", fname, err));
    }
    let rx = broadcaster.rx.clone();
    thread::spawn(move || loop {
        let bm = rx.recv().expect("receive broadcast message");
        if bm.should_notify(CLIENT_ID) {
            println!("{}", &bm.message);
        }
    });

    let stdin = io::stdin();
    for maybe_line in stdin.lock().lines() {
        match maybe_line {
            Ok(line) => match editor.handle(CLIENT_ID, &line) {
                Ok(response) => println!("{}", &response),
                Err(e) => eprintln!("{}: {:?}", e, line),
            },
            Err(e) => eprintln!("failed to read line: {}", e),
        }
    }

    editor.remove_client(CLIENT_ID);
}
