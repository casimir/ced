use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, LineWriter, Read, Write};
use std::path::PathBuf;
use std::rc::Rc;

use jsonrpc_lite::JsonRpc;
use mio::tcp::TcpListener;
use mio::Evented;
use mio::{Events, Poll, PollOpt, Ready, Token};
#[cfg(unix)]
use mio_uds::UnixListener;
use serde_json;

use editor::Editor;

#[derive(Debug)]
pub enum ServerMode {
    Tcp(String),
    UnixSocket(String),
}

pub struct SessionManager {
    root: PathBuf,
}

impl SessionManager {
    pub fn new() -> SessionManager {
        let mut app_dir = env::temp_dir();
        app_dir.push("ced");
        app_dir.push(env::var("LOGNAME").unwrap_or("anon".into()));
        SessionManager { root: app_dir }
    }

    pub fn ensure_root_dir(&self) -> io::Result<()> {
        if !&self.root.exists() {
            fs::create_dir_all(&self.root)
        } else {
            Ok(())
        }
    }

    fn session_full_path(&self, name: &str) -> PathBuf {
        let mut session_path = self.root.clone();
        session_path.push(name);
        session_path
    }

    pub fn exists(&self, name: &str) -> bool {
        let session_path = self.session_full_path(name);
        session_path.exists()
    }

    pub fn list(&self) -> Vec<String> {
        match fs::read_dir(&self.root) {
            Ok(entries) => entries
                .filter_map(|entry| {
                    entry.ok().and_then(|e| {
                        e.path()
                            .file_name()
                            .and_then(|n| n.to_str().map(|s| String::from(s)))
                    })
                })
                .collect::<Vec<String>>(),
            Err(_) => Vec::new(),
        }
    }

    pub fn remove(&self, name: &str) -> io::Result<()> {
        let session_path = self.session_full_path(name);
        if session_path.exists() {
            fs::remove_file(session_path)?;
            if self.list().len() == 0 {
                fs::remove_dir(&self.root)
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }
}

trait Stream: Evented + Read + Write {}

impl<T> Stream for T
where
    T: Evented + Read + Write,
{
}

enum Listener {
    Tcp(TcpListener),
    #[cfg(unix)]
    Unix(UnixListener),
}

impl Listener {
    fn inner(&self) -> &Evented {
        use self::Listener::*;
        match self {
            Tcp(inner) => inner,
            #[cfg(unix)]
            Unix(inner) => inner,
        }
    }

    fn accept(&self) -> ::std::io::Result<Box<Stream>> {
        use self::Listener::*;
        match self {
            Tcp(inner) => {
                let (stream, _) = inner.accept()?;
                Ok(Box::new(stream))
            }
            #[cfg(unix)]
            Unix(inner) => {
                let opt = inner.accept()?;
                // None when no connection is waiting to be accepted
                let (stream, _) = opt.unwrap();
                Ok(Box::new(stream))
            }
        }
    }
}

struct Connection<'a> {
    handle: Box<Stream + 'a>,
}

impl<'a> Connection<'a> {
    fn new(handle: Box<Stream>) -> Connection<'a> {
        Connection { handle: handle }
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
    }
}

pub struct Server {
    pub mode: ServerMode,
    sessions: SessionManager,
}

impl Server {
    pub fn new(mode: ServerMode) -> Server {
        let server = Server {
            mode: mode,
            sessions: SessionManager::new(),
        };
        server
    }

    fn make_listener(&self) -> Listener {
        match &self.mode {
            ServerMode::Tcp(addr) => {
                let sock_addr = addr.parse().unwrap();
                Listener::Tcp(TcpListener::bind(&sock_addr).unwrap())
            }
            #[cfg(unix)]
            ServerMode::UnixSocket(name) => {
                self.sessions
                    .ensure_root_dir()
                    .expect("could not create the session directory");
                let path = self.sessions.session_full_path(name);
                Listener::Unix(UnixListener::bind(path).unwrap())
            }
            #[cfg(not(unix))]
            _ => unimplemented!(),
        }
    }

    fn write_message(
        &self,
        conn: &mut Connection,
        message: &JsonRpc,
    ) -> Result<(), serde_json::Error> {
        let json = serde_json::to_value(message)?;
        let writer = LineWriter::new(conn.handle.as_mut());
        serde_json::to_writer(writer, &json)
    }

    pub fn run(&self, filenames: Vec<&str>) -> Result<(), Error> {
        let mut editor = Editor::new(filenames);
        let listener = self.make_listener();
        let poll = Poll::new().unwrap();
        let mut next_client_id = 1;
        let connections = Rc::new(RefCell::new(HashMap::new()));
        let mut events = Events::with_capacity(128);

        poll.register(
            listener.inner(),
            Token(0),
            Ready::readable(),
            PollOpt::edge(),
        ).unwrap();
        loop {
            poll.poll(&mut events, None).unwrap();

            for event in events.iter() {
                match event.token() {
                    Token(0) => {
                        let stream = listener
                            .accept()
                            .expect("error while accepting a connection");
                        poll.register(
                            stream.as_ref(),
                            Token(next_client_id),
                            Ready::readable(),
                            PollOpt::edge(),
                        ).unwrap();

                        println!("new connection (client {})", next_client_id);
                        let conn = Connection::new(stream);
                        match editor.add_client(next_client_id) {
                            Ok((message, broadcast)) => {
                                connections.borrow_mut().insert(next_client_id, conn);
                                let mut conns = connections.borrow_mut();
                                if let Some(msg) = &broadcast {
                                    let errors: Vec<
                                        serde_json::Error,
                                    > = conns
                                        .iter_mut()
                                        .map(|(_, c)| self.write_message(c, msg))
                                        .filter_map(Result::err)
                                        .collect();
                                }

                                let conn = conns.get_mut(&next_client_id).unwrap();
                                self.write_message(conn, &message).expect(&format!(
                                    "could not send init message to client {}",
                                    next_client_id
                                ));
                                next_client_id += 1;
                            }
                            Err(e) => {
                                poll.deregister(conn.handle.as_ref()).unwrap();
                                eprintln!("could not connect client: {}", e);
                            }
                        }
                    }
                    Token(client_id) => {
                        let mut line = String::new();
                        // need to get stream in a seperate scope in case client closes the connection
                        // in which case we want to drop it
                        {
                            {
                                let mut conns = connections.borrow_mut();
                                let mut conn = conns.get_mut(&client_id).unwrap();
                                let mut stream = conn.handle.as_mut();
                                let mut reader = BufReader::new(stream);
                                match reader.read_line(&mut line) {
                                    Ok(m) => m,
                                    Err(e) => {
                                        if e.kind() == ::std::io::ErrorKind::WouldBlock {
                                            // avoid false positive
                                            continue;
                                        } else {
                                            panic!(
                                                "got an error when reading from connection: {}",
                                                e
                                            )
                                        }
                                    }
                                };
                            }
                            if !line.is_empty() {
                                match editor.handle(client_id, &line) {
                                    Ok((message, broadcast)) => {
                                        let mut conns = connections.borrow_mut();
                                        if let Some(msg) = &broadcast {
                                            let errors: Vec<
                                        serde_json::Error,
                                    > = conns
                                        .iter_mut()
                                        .map(|(_, c)| self.write_message(c, msg))
                                        .filter_map(Result::err)
                                        .collect();
                                        }

                                        let mut conn = conns.get_mut(&client_id).unwrap();
                                        self.write_message(conn, &message).expect(&format!(
                                            "could not send message to client {}",
                                            client_id
                                        ));
                                    }
                                    Err(e) => eprintln!("{}: {:?}", e, line),
                                }
                            }
                        }
                        if line.is_empty() {
                            eprintln!("client closed connection");
                            let conn = connections.borrow_mut().remove(&client_id).unwrap();
                            poll.deregister(*&conn.handle.as_ref()).unwrap();
                        }
                    }
                }
            }
            if next_client_id > 0 && connections.borrow().len() == 0 {
                println!("no more client, exiting...");
                break;
            }
        }
        if let ServerMode::UnixSocket(name) = &self.mode {
            self.sessions
                .remove(&name)
                .expect(&format!("could not remove session {}", name));
        }
        Ok(())
    }
}
