extern crate crossbeam_channel;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate failure;
extern crate mio;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[cfg(windows)]
extern crate mio_named_pipes;
#[cfg(unix)]
extern crate mio_uds;
#[cfg(windows)]
extern crate mio_uds_windows;
extern crate serde_json;
#[cfg(windows)]
extern crate winapi;

mod client;
mod connection;
pub mod jsonrpc;
pub mod protocol;
mod session;
mod transport;

use std::env;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use failure::Error;

pub use self::client::{Client, Events, StdioClient};
pub use self::connection::{Connection, ConnectionEvent, ConnectionState, Menu};
pub use self::jsonrpc::{ClientEvent, Id, Request};
pub use self::session::{ConnectionMode, Session};
pub use self::transport::{EventedStream, ServerListener, ServerStream, Stream};

pub fn find_bin() -> String {
    env::var("CED_BIN").unwrap_or(
        env::current_exe()
            .map(|mut exe| {
                exe.pop();
                if cfg!(windows) {
                    exe.push("ced.exe");
                } else {
                    exe.push("ced");
                }
                if exe.exists() {
                    exe.display().to_string()
                } else {
                    String::from("ced")
                }
            })
            .unwrap_or(String::from("ced")),
    )
}

pub fn start_daemon(session: &Session) -> Result<u32, Error> {
    let bin = find_bin();
    let session_arg = format!("--session={}", session.mode);
    let args = vec!["--mode=server", &session_arg];
    let prg = Command::new(&bin)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let pid = prg.id();

    //  Once ready to accept connections the server send an empty line on stdout.
    if let Some(stdout) = prg.stdout {
        for line in BufReader::new(stdout).lines() {
            let should_stop = match line {
                Ok(l) => l.is_empty(),
                Err(err) => {
                    error!("failed to read stdout: {}", err);
                    true
                }
            };
            if should_stop {
                break;
            }
        }
    } else {
        error!("could not capture stdout");
    }

    info!("server command: {} {:?}", bin, args);
    Ok(pid)
}

pub fn ensure_session(session: &Session) -> Result<(), Error> {
    if let ConnectionMode::Socket(path) = &session.mode {
        if !path.exists() {
            start_daemon(&session)?;
        }
    }
    Ok(())
}
