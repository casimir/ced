use std::io::{BufRead, Write};

use jsonrpc_lite::JsonRpc;
use serde_json;

use editor::Editor;

fn serialize_message(message: &JsonRpc) -> String {
    let json = serde_json::to_value(message).unwrap();
    serde_json::to_string(&json).unwrap()
}

pub fn start_standalone(
    input: &mut BufRead,
    output: &mut Write,
    error: &mut Write,
    filenames: &[&str],
) {
    let mut editor = Editor::new("", &filenames);
    let (response, _) = editor.add_client(1).unwrap();
    writeln!(output, "{}", serialize_message(&response)).expect("write error");

    let mut buf = String::new();
    while let Ok(n) = input.read_line(&mut buf) {
        if n == 0 {
            break;
        }
        match editor.handle(1, &buf) {
            Ok((response, broadcast)) => {
                if let Some(msg) = broadcast {
                    println!("{}", serialize_message(&msg));
                }
                writeln!(output, "{}", serialize_message(&response)).expect("write error");
            }
            Err(e) => {
                writeln!(error, "{}: {:?}", e, buf).expect("write error");
            }
        }
        buf.clear();
    }
    let _ = editor.remove_client(1);
}
