use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind::WouldBlock;
use std::io::{BufRead, BufReader, Write};
use std::rc::Rc;

use jsonrpc_lite::JsonRpc;
use mio::{Events, Poll, PollOpt, Ready, Token};
use serde_json;

use editor::Editor;
use remote::{ConnectionMode, Error, EventedStream, Listener, Result, Session};

struct Connection<'a> {
    handle: Box<EventedStream + 'a>,
}

impl<'a> Connection<'a> {
    fn new(handle: Box<EventedStream>) -> Connection<'a> {
        Connection { handle }
    }
}

pub struct Server {
    session: Session,
}

impl Server {
    pub fn new(session: Session) -> Server {
        Server { session }
    }

    fn write_message(&self, conn: &mut Connection, message: &JsonRpc) -> Result<()> {
        let json = serde_json::to_value(message)?;
        let payload = serde_json::to_string(&json)? + "\n";
        trace!("-> {:?}", payload);
        conn.handle.write_all(payload.as_bytes())?;
        Ok(())
    }

    pub fn run(&self, filenames: &[&str]) -> Result<()> {
        let mut editor = Editor::new(&format!("{}", self.session), filenames);
        let listener = Listener::new(&self.session)?;
        let poll = Poll::new()?;
        let mut next_client_id = 1;
        let connections = Rc::new(RefCell::new(HashMap::new()));
        let mut events = Events::with_capacity(1024);

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
                        match editor.add_client(next_client_id) {
                            Ok((message, broadcast)) => {
                                connections.borrow_mut().insert(next_client_id, conn);
                                let mut conns = connections.borrow_mut();
                                if let Some(msg) = &broadcast {
                                    let errors: Vec<Error> = conns
                                        .iter_mut()
                                        .map(|(_, c)| self.write_message(c, msg))
                                        .filter_map(Result::err)
                                        .collect();
                                    for e in &errors {
                                        error!("{}", e)
                                    }
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
                                error!("could not connect client: {}", e);
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
                                    Ok((message, broadcast)) => {
                                        let mut conns = connections.borrow_mut();
                                        if let Some(msg) = &broadcast {
                                            let errors: Vec<
                                        Error,
                                    > = conns
                                        .iter_mut()
                                        .map(|(_, c)| self.write_message(c, msg))
                                        .filter_map(Result::err)
                                        .collect();
                                            for e in &errors {
                                                error!("{}", e)
                                            }
                                        }

                                        let mut conn = conns.get_mut(&client_id).unwrap();
                                        self.write_message(conn, &message).expect(&format!(
                                            "could not send message to client {}",
                                            client_id
                                        ));
                                    }
                                    Err(e) => error!("{}: {:?}", e, line),
                                }
                            }
                        }
                        if line.is_empty() {
                            let conn = connections.borrow_mut().remove(&client_id).unwrap();
                            poll.deregister(*&conn.handle.as_ref()).unwrap();
                            info!("client {} disconnected", client_id);
                        }
                    }
                }
            }
            if next_client_id > 0 && connections.borrow().len() == 0 {
                info!("no more client, exiting...");
                break;
            }
        }
        if let ConnectionMode::Socket(path) = &self.session.mode {
            fs::remove_file(path).expect(&format!("could not clean session {}", self.session));
            if Session::list().is_empty() {
                fs::remove_dir(path.parent().unwrap())
                    .unwrap_or_else(|e| warn!("could not clean session directory: {}", e));
            }
        }
        Ok(())
    }
}
