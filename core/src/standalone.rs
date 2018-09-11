use std::io::{BufRead, Write};

use failure::Error;

use editor::Editor;
use server::Broadcaster;

const CLIENT_ID: usize = 1;

pub fn start_standalone(
    input: &mut BufRead,
    output: &mut Write,
    error: &mut Write,
    filenames: &[&str],
) -> Result<(), Error> {
    let broadcaster = Broadcaster::new();
    let mut editor = Editor::new("", &filenames, broadcaster.tx);
    writeln!(output, "{}", editor.add_client(1).unwrap())?;

    let mut buf = String::new();
    while let Ok(n) = input.read_line(&mut buf) {
        if n == 0 {
            break;
        }
        match editor.handle(CLIENT_ID, &buf) {
            Ok(response) => {
                while let Ok(bm) = broadcaster.rx.try_recv() {
                    if !bm.skiplist.contains(&CLIENT_ID) {
                        writeln!(output, "{}", &bm.message)?;
                    }
                }
                writeln!(output, "{}", &response)?;
            }
            Err(e) => {
                writeln!(error, "{}: {:?}", e, buf)?;
            }
        }
        buf.clear();
    }
    let _ = editor.remove_client(1);
    Ok(())
}
