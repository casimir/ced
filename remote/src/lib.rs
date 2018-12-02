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
pub mod jsonrpc;
pub mod protocol;
mod session;
mod transport;

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};

use crossbeam_channel as channel;
use failure::Error;

pub use client::{Client, Events, StdioClient};
pub use jsonrpc::{ClientEvent, Id, Request};
pub use session::{ConnectionMode, Session};
pub use transport::{EventedStream, ServerListener, ServerStream, Stream};

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

#[derive(Clone, Debug, Default)]
pub struct ConnectionState {
    pub session: String,
    pub view: protocol::notification::view::Params,
}

impl ConnectionState {
    fn update_context(&mut self, event: &ClientEvent) {
        if let ClientEvent::Notification(notif) = event {
            use protocol::notification::*;
            match notif.method.as_str() {
                "info" => {
                    if let Ok(Some(params)) = notif.params::<info::Params>() {
                        self.session = params.session;
                    }
                }
                "view" => {
                    if let Ok(Some(params)) = notif.params::<view::Params>() {
                        self.view = params;
                    }
                }
                _ => {}
            }
        }
    }
}

pub struct Connection {
    client: Client,
    state_lock: Arc<RwLock<ConnectionState>>,
    requests: channel::Sender<Request>,
    next_request_id: i32,
    pub pending: HashMap<Id, Request>,
}

impl Connection {
    pub fn new(session: &Session) -> Result<Connection, Error> {
        let (client, requests) = Client::new(session)?;
        Ok(Connection {
            client,
            state_lock: Arc::new(RwLock::new(ConnectionState::default())),
            requests,
            next_request_id: 0,
            pending: HashMap::new(),
        })
    }

    pub fn state(&self) -> ConnectionState {
        self.state_lock.read().unwrap().clone()
    }

    pub fn connect(&self) -> channel::Receiver<ClientEvent> {
        let events = self.client.run();
        let (tx, rx) = channel::unbounded();
        let ctx_lock = self.state_lock.clone();
        std::thread::spawn(move || {
            for ev in events {
                match ev {
                    Ok(e) => {
                        let mut ctx = ctx_lock.write().unwrap();
                        ctx.update_context(&e);
                        tx.send(e).expect("send event");
                    }
                    Err(e) => error!("{}", e),
                }
            }
        });
        rx
    }

    pub fn request_id(&mut self) -> Id {
        let id = self.next_request_id;
        self.next_request_id += 1;
        Id::Number(id)
    }

    pub fn request(&mut self, message: Request) {
        self.pending.insert(message.id.clone(), message.clone());
        self.requests.send(message).expect("send request");
    }
}
