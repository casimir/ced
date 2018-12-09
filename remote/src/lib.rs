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

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use failure::Error;

pub use self::client::{Client, Events, StdioClient};
pub use self::connection::{Connection, ConnectionState};
pub use self::jsonrpc::{ClientEvent, Id, Request};
pub use self::session::{ConnectionMode, Session};
pub use self::transport::{EventedStream, ServerListener, ServerStream, Stream};

pub fn start_daemon(command: &str, session: &Session) -> Result<u32, Error> {
    let session_arg = format!("--session={}", session.mode);
    let args = vec![command, "--mode=server", &session_arg];
    let prg = Command::new(&args[0])
        .args(&args[1..])
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

    info!("server command: {:?}", args);
    Ok(pid)
}

pub fn ensure_session(command: &str, session: &Session) -> Result<(), Error> {
    if let ConnectionMode::Socket(path) = &session.mode {
        if !path.exists() {
            start_daemon(command, &session)?;
        }
    }
    Ok(())
}
