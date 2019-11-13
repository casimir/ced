use std::fmt;
use std::fs::read_to_string;
use std::io::{self, Read};
use std::path::Path;

use crate::editor::Editor;
use crate::server::Broadcaster;

const CLIENT_ID: usize = 1;

pub fn exec_scripts(filenames: &[&str]) -> io::Result<()> {
    let broadcaster = Broadcaster::default();
    let mut editor = Editor::new("", broadcaster.tx);

    editor.add_client(CLIENT_ID);

    for fname in filenames {
        let source = if *fname == "-" {
            let mut buffer = String::new();
            let mut stdin = io::stdin();
            stdin.read_to_string(&mut buffer)?;
            buffer
        } else {
            read_to_string(fname)?
        };
        editor
            .exec_lua(fname, CLIENT_ID, |lua| lua.load(&source).exec())
            .map_err(|e| {
                eprintln!("lua error: {}: {}", fname, e.to_string());
                io::Error::new(io::ErrorKind::Other, "invalid lua script")
            })?;
    }

    editor.remove_client(CLIENT_ID);
    Ok(())
}

#[derive(Debug)]
pub enum LuaIOError {
    Io(io::Error),
    Lua(rlua::Error),
}

impl fmt::Display for LuaIOError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use LuaIOError::*;
        match self {
            Io(e) => write!(f, "{}", e),
            Lua(e) => write!(f, "{}", e),
        }
    }
}

pub fn exec_script_oneshot<P: AsRef<Path>>(path: P) -> Result<(), LuaIOError> {
    let broadcaster = Broadcaster::default();
    let mut editor = Editor::new("", broadcaster.tx);

    editor.add_client(CLIENT_ID);

    let source = read_to_string(&path).map_err(LuaIOError::Io)?;
    editor
        .exec_lua(&path.as_ref().display().to_string(), CLIENT_ID, |lua| {
            lua.load(&source).exec()
        })
        .map_err(LuaIOError::Lua)?;

    editor.remove_client(CLIENT_ID);
    Ok(())
}
