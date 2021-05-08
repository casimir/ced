#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde;

mod client;
mod connection;
pub mod jsonrpc;
mod keys;
pub mod protocol;
mod session;
mod transport;

use std::env;
use std::io::{self, BufRead, BufReader};
use std::process::{Command, Stdio};

pub use self::client::{Client, ClientEventResult, ClientEventStream};
pub use self::connection::{Connection, ConnectionEvent, ConnectionState, Menu};
pub use self::jsonrpc::{ClientEvent, Id, Request};
pub use self::session::{ConnectionMode, Session};
pub use self::transport::{ServerListener, ServerStream};

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

pub fn start_daemon(session: &Session) -> io::Result<u32> {
    let bin = find_bin();
    let session_arg = format!("--session={}", session.mode);
    let args = vec!["--mode=server", &session_arg];
    let stderr = if std::env::var("CED_FWSTDERR") == Ok(String::from("1")) {
        log::debug!("forwarding stderr from daemon to client");
        Stdio::inherit()
    } else {
        Stdio::null()
    };
    let prg = Command::new(&bin)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()?;
    let pid = prg.id();

    //  Once ready to accept connections the server send an empty line on stdout.
    if let Some(stdout) = prg.stdout {
        for line in BufReader::new(stdout).lines() {
            let should_stop = match line {
                Ok(l) => l.is_empty(),
                Err(err) => {
                    log::error!("failed to read stdout: {}", err);
                    true
                }
            };
            if should_stop {
                break;
            }
        }
    } else {
        log::error!("could not capture stdout");
    }

    log::info!("server command: {} {:?}", bin, args);
    Ok(pid)
}

pub fn ensure_session(session: &Session) -> io::Result<()> {
    if let ConnectionMode::Socket(path) = &session.mode {
        if !path.exists() {
            start_daemon(&session)?;
        }
    }
    Ok(())
}
