use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind::WouldBlock;
use std::io::{self, BufRead, BufReader, Write};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use failure::Error;
use mio::{Events, Poll, PollOpt, Ready, Registration, Token};

use editor::Editor;
use remote::protocol::Object;
use remote::{ConnectionMode, EventedStream, ServerListener, Session};

#[derive(Debug)]
pub struct BroadcastMessage {
    pub message: Object,
    pub skiplist: Vec<usize>,
}

impl BroadcastMessage {
    pub fn new_skip(message: Object, skiplist: Vec<usize>) -> BroadcastMessage {
        BroadcastMessage { message, skiplist }
    }

    pub fn new(message: Object) -> BroadcastMessage {
        Self::new_skip(message, Vec::new())
    }
}

pub struct Broadcaster {
    registration: Registration,
    pub tx: mpsc::Sender<BroadcastMessage>,
    pub rx: mpsc::Receiver<BroadcastMessage>,
}

impl Broadcaster {
    pub fn new() -> Broadcaster {
        let (registration, set_readiness) = Registration::new2();
        let (tx, inner_rx) = mpsc::channel();
        let (inner_tx, rx) = mpsc::channel();
        thread::spawn(move || loop {
            let message = inner_rx.recv().expect("receive broadcast message");
            inner_tx
                .send(message)
                .expect("transmit broadcasted message");
            set_readiness
                .set_readiness(Ready::readable())
                .expect("set broadcast queue readable")
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
            fs::remove_file(path).expect(&format!("could not clean session {}", self.session));
            if Session::list().is_empty() {
                fs::remove_dir(path.parent().unwrap())
                    .unwrap_or_else(|e| warn!("could not clean session directory: {}", e));
            }
        }
    }

    fn write_message(
        &self,
        client_id: usize,
        conn: &mut Connection,
        message: &Object,
    ) -> Result<(), io::Error> {
        trace!("-> ({}) {}", client_id, message);
        conn.handle.write_fmt(format_args!("{}\n", message))
    }

    pub fn run(&self, filenames: &[&str]) -> Result<(), Error> {
        let broadcaster = Broadcaster::new();
        let mut editor = Editor::new(&self.session.to_string(), filenames, broadcaster.tx);
        let listener = ServerListener::new(&self.session)?;
        let poll = Poll::new()?;
        let mut next_client_id = FIRST_CLIENT_ID;
        let connections = Rc::new(RefCell::new(HashMap::new()));
        let mut events = Events::with_capacity(1024);

        poll.register(listener.inner(), SERVER, Ready::readable(), PollOpt::edge())
            .unwrap();
        poll.register(
            &broadcaster.registration,
            BROADCAST,
            Ready::readable(),
            PollOpt::edge(),
        ).unwrap();
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
                        ).unwrap();

                        info!("client {} connected", next_client_id);
                        let conn = Connection::new(stream);
                        // TODO check if ping or real client
                        match editor.add_client(next_client_id) {
                            Ok(message) => {
                                connections.borrow_mut().insert(next_client_id, conn);
                                let mut conns = connections.borrow_mut();
                                let conn = conns.get_mut(&next_client_id).unwrap();
                                self.write_message(next_client_id, conn, &message).expect(
                                    &format!(
                                        "could not send init message to client {}",
                                        next_client_id
                                    ),
                                );
                                next_client_id += 1;
                            }
                            Err(e) => {
                                poll.deregister(conn.handle.as_ref()).unwrap();
                                error!("could not connect client: {}", e);
                            }
                        }
                    }
                    BROADCAST => {
                        while let Ok(bm) = broadcaster.rx.try_recv() {
                            let mut conns = connections.borrow_mut();
                            let errors: Vec<Error> = conns
                                .iter_mut()
                                .filter(|(client_id, _)| !bm.skiplist.contains(&client_id))
                                .map(|(client_id, c)| {
                                    self.write_message(*client_id, c, &bm.message)
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
                                let mut conn = conns.get_mut(&client_id).unwrap();
                                let mut stream = conn.handle.as_mut();
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
                                        let mut conn = conns.get_mut(&client_id).unwrap();
                                        self.write_message(client_id, conn, &message).expect(
                                            &format!(
                                                "could not send message to client {}",
                                                client_id
                                            ),
                                        );
                                    }
                                    Err(e) => error!("{}: {:?}", e, line),
                                }
                            }
                        }
                        if line.is_empty() {
                            let conn = connections.borrow_mut().remove(&client_id).unwrap();
                            poll.deregister(conn.handle.as_ref()).unwrap();
                            info!("client {} disconnected", client_id);
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
