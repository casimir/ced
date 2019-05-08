use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::ErrorKind::WouldBlock;
use std::io::{self, BufRead, BufReader, Write};
use std::rc::Rc;
use std::thread;

use crossbeam_channel as channel;
use failure::Error;
use mio::{Events, Poll, PollOpt, Ready, Registration, Token};

use crate::editor::Editor;
use remote::jsonrpc::Notification;
use remote::{ConnectionMode, EventedStream, ServerListener, Session};

#[derive(Debug)]
pub struct BroadcastMessage {
    pub message: Notification,
    clients: Option<Vec<usize>>,
}

impl BroadcastMessage {
    pub fn new(message: Notification) -> BroadcastMessage {
        BroadcastMessage {
            message,
            clients: None,
        }
    }

    pub fn for_clients(clients: Vec<usize>, message: Notification) -> BroadcastMessage {
        BroadcastMessage {
            message,
            clients: Some(clients),
        }
    }

    #[inline]
    pub fn should_notify(&self, client_id: usize) -> bool {
        match &self.clients {
            Some(cs) => cs.contains(&client_id),
            None => true,
        }
    }
}

pub struct Broadcaster {
    registration: Registration,
    pub tx: channel::Sender<BroadcastMessage>,
    pub rx: channel::Receiver<BroadcastMessage>,
}

impl Default for Broadcaster {
    fn default() -> Self {
        let (registration, set_readiness) = Registration::new2();
        let (tx, inner_rx) = channel::unbounded();
        let (inner_tx, rx) = channel::unbounded();
        thread::spawn(move || loop {
            if let Ok(message) = inner_rx.recv() {
                inner_tx.send(message).expect("send message");
                set_readiness
                    .set_readiness(Ready::readable())
                    .expect("set broadcast queue readable")
            }
        });
        Broadcaster {
            registration,
            tx,
            rx,
        }
    }
}

struct Connection<'a> {
    handle: Box<EventedStream + 'a>,
}

impl<'a> Connection<'a> {
    fn new(handle: Box<EventedStream>) -> Connection<'a> {
        Connection { handle }
    }
}

const SERVER: Token = Token(0);
const BROADCAST: Token = Token(1);
const FIRST_CLIENT_ID: usize = 2;

pub struct Server {
    session: Session,
}

impl Server {
    pub fn new(session: Session) -> Server {
        Server { session }
    }

    fn cleanup(&self) {
        if let ConnectionMode::Socket(path) = &self.session.mode {
            fs::remove_file(path).expect("clean session");
            if Session::list().is_empty() {
                fs::remove_dir(path.parent().unwrap())
                    .unwrap_or_else(|e| warn!("could not clean session directory: {}", e));
            }
        }
    }

    fn write_message<T>(
        &self,
        client_id: usize,
        conn: &mut Connection,
        message: &T,
    ) -> Result<(), io::Error>
    where
        T: fmt::Display,
    {
        trace!("-> ({}) {}", client_id, message);
        conn.handle.write_fmt(format_args!("{}\n", message))
    }

    pub fn run(&self) -> Result<(), Error> {
        let broadcaster = Broadcaster::default();
        let mut editor = Editor::new(&self.session.to_string(), broadcaster.tx);
        let listener = ServerListener::new(&self.session)?;
        let poll = Poll::new()?;
        let mut next_client_id = FIRST_CLIENT_ID;
        let connections = Rc::new(RefCell::new(HashMap::new()));
        let mut events = Events::with_capacity(1024);

        // TODO remove `.inner()`
        poll.register(listener.inner(), SERVER, Ready::readable(), PollOpt::edge())
            .unwrap();
        poll.register(
            &broadcaster.registration,
            BROADCAST,
            Ready::readable(),
            PollOpt::edge(),
        )
        .unwrap();
        // notify readiness to a potential awaiting client
        println!();
        loop {
            poll.poll(&mut events, None).unwrap();
            for event in events.iter() {
                match event.token() {
                    SERVER => {
                        let stream = match listener.accept() {
                            Ok(s) => s,
                            Err(e) => {
                                if e.kind() == WouldBlock {
                                    continue;
                                } else {
                                    panic!("error while accepting a connection: {}", e)
                                }
                            }
                        };
                        poll.register(
                            stream.as_ref(),
                            Token(next_client_id),
                            Ready::readable(),
                            PollOpt::edge(),
                        )
                        .unwrap();

                        info!("client {} connected", next_client_id);
                        let conn = Connection::new(stream);
                        // TODO check if ping or real client
                        connections.borrow_mut().insert(next_client_id, conn);
                        editor.add_client(next_client_id);
                        next_client_id += 1;
                    }
                    BROADCAST => {
                        while let Ok(bm) = broadcaster.rx.try_recv() {
                            let mut conns = connections.borrow_mut();
                            let errors: Vec<Error> = conns
                                .iter_mut()
                                .filter(|(&client_id, _)| bm.should_notify(client_id))
                                .map(|(&client_id, c)| {
                                    self.write_message(client_id, c, &bm.message)
                                })
                                .filter_map(Result::err)
                                .map(Error::from)
                                .collect();
                            for e in &errors {
                                error!("{}", e)
                            }
                        }
                    }
                    Token(client_id) => {
                        trace!("read event for {}", client_id);
                        let mut line = String::new();
                        {
                            {
                                let mut conns = connections.borrow_mut();
                                let conn = conns.get_mut(&client_id).unwrap();
                                let stream = conn.handle.as_mut();
                                let mut reader = BufReader::new(stream);
                                if let Err(e) = reader.read_line(&mut line) {
                                    if e.kind() == WouldBlock {
                                        continue;
                                    } else {
                                        error!("error while reading from connection: {:?}", e);
                                    }
                                };
                            }
                            if !line.is_empty() {
                                match editor.handle(client_id, &line) {
                                    Ok(message) => {
                                        let mut conns = connections.borrow_mut();
                                        let conn = conns.get_mut(&client_id).unwrap();
                                        self.write_message(client_id, conn, &message)
                                            .expect("send response to client");
                                    }
                                    Err(e) => error!("{}: {:?}", e, line),
                                }
                            }
                        }
                        if line.is_empty() {
                            editor.remove_client(client_id);
                            let conn = connections.borrow_mut().remove(&client_id).unwrap();
                            poll.deregister(conn.handle.as_ref()).unwrap();
                            info!("client {}: connection lost", client_id);
                        }
                        for client_id in editor.removed_clients() {
                            let conn = connections.borrow_mut().remove(&client_id).unwrap();
                            poll.deregister(conn.handle.as_ref()).unwrap();
                            info!("client {}: quit", client_id);
                        }
                    }
                }
            }
            if next_client_id > FIRST_CLIENT_ID && connections.borrow().len() == 0 {
                info!("no more client, exiting...");
                break;
            }
        }
        self.cleanup();
        Ok(())
    }
}
