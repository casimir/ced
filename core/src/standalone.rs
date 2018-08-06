use std::io::{BufRead, Write};

use failure::Error;

use editor::Editor;

pub fn start_standalone(
    input: &mut BufRead,
    output: &mut Write,
    error: &mut Write,
    filenames: &[&str],
) -> Result<(), Error> {
    let mut editor = Editor::new("", &filenames);
    let (response, _) = editor.add_client(1).unwrap();
    writeln!(output, "{}", &response)?;

    let mut buf = String::new();
    while let Ok(n) = input.read_line(&mut buf) {
        if n == 0 {
            break;
        }
        match editor.handle(1, &buf) {
            Ok((response, broadcast)) => {
                if let Some(msg) = broadcast {
                    writeln!(output, "{}", &msg)?;
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
