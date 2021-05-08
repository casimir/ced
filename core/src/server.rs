use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::sync::{Arc, RwLock};

use crate::editor::Editor;
use async_channel::{unbounded, Receiver, Sender};
use async_executor::LocalExecutor;
use futures_lite::*;
use remote::jsonrpc::Notification;
use remote::{ConnectionMode, ServerListener, ServerStream, Session};

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

const FIRST_CLIENT_ID: usize = 1;

#[derive(Clone, Debug)]
enum Event {
    Join((usize, ServerStream)),
    Leave(usize),
    Message((usize, String)),
}

pub struct Server {
    session: Session,
}

impl Server {
    pub fn new(session: Session) -> Server {
        Server { session }
    }

    fn write_client<T>(client_id: usize, stream: &mut ServerStream, message: &T) -> io::Result<()>
    where
        T: fmt::Display,
    {
        log::trace!("-> ({}) {}", client_id, message);
        future::block_on(stream.write_all(format!("{}\n", message).as_bytes()))
    }

    async fn handle_events(ex: Arc<LocalExecutor<'_>>, session: String, receiver: Receiver<Event>) {
        let (bsender, breceiver) = unbounded();
        let mut editor = Editor::new(&session, bsender);
        let clients = Arc::new(RwLock::new(HashMap::<usize, ServerStream>::new()));

        let bclients = Arc::clone(&clients);
        log::trace!("spawning broadcast handler task");
        ex.spawn(async move {
            log::trace!("spawned broadcast handler task");
            while let Ok(bm) = breceiver.recv().await {
                log::trace!("broadcast event: {:?}", bm);
                let mut index = bclients.write().expect("lock client index");
                for (client_id, stream) in index.iter_mut() {
                    if bm.should_notify(*client_id) {
                        let res = Self::write_client(*client_id, stream, &bm.message);
                        if let Err(e) = res {
                            log::error!("{}", e);
                        }
                    }
                }
            }
        })
        .detach();

        log::trace!("starting client event loop");
        while let Ok(event) = receiver.recv().await {
            log::trace!("client event: {:?}", event);
            let mut is_leave_event = false;
            match event {
                Event::Join((client_id, stream)) => {
                    clients
                        .write()
                        .expect("lock client index")
                        .insert(client_id, stream);
                    log::info!("client {} connected", client_id);
                    // TODO check if ping or real client
                    editor.add_client(client_id);
                }
                Event::Leave(client_id) => {
                    editor.remove_client(client_id);
                    clients
                        .write()
                        .expect("lock client index")
                        .remove(&client_id)
                        .expect("remove client from index");
                    log::info!("client {}: connection closed", client_id);
                    is_leave_event = true;
                }
                Event::Message((client_id, raw)) => match editor.handle(client_id, &raw) {
                    Ok(message) => {
                        let mut index = clients.write().expect("lock client index");
                        let stream = index.get_mut(&client_id).unwrap();
                        Self::write_client(client_id, stream, &message)
                            .expect("send response to client");
                    }
                    Err(e) => log::error!("{}: {:?}", e, raw),
                },
            }
            for client_id in editor.removed_clients() {
                clients
                    .write()
                    .expect("lock client index")
                    .remove(&client_id)
                    .expect("remove client from index");
                log::info!("client {}: quit", client_id);
            }
            if clients.read().expect("lock client index").len() == 0 && is_leave_event {
                break;
            }
        }
    }

    async fn read_client(
        client_id: usize,
        stream: ServerStream,
        sender: Sender<Event>,
    ) -> io::Result<()> {
        let mut lines = io::BufReader::new(stream).lines();
        while let Some(line) = lines.next().await {
            if line.is_err() {
                // connection reset
                break;
            }
            log::trace!("read event for {}: {:?}", client_id, line);
            sender
                .send(Event::Message((client_id, line.unwrap())))
                .await
                .unwrap_or_else(|e| log::error!("{}", e));
        }
        log::trace!("ending connection handler: {}", client_id);
        sender
            .send(Event::Leave(client_id))
            .await
            .unwrap_or_else(|e| log::error!("{}", e));
        Ok(())
    }

    async fn serve(
        ex: Arc<LocalExecutor<'_>>,
        session: Session,
        sender: Sender<Event>,
    ) -> io::Result<()> {
        let mut next_client_id = FIRST_CLIENT_ID;
        let listener = ServerListener::bind(&session).await?;
        let mut incoming = listener.incoming();
        // notify readiness to a potential awaiting client
        println!();
        log::trace!("ready to accept incoming connections");
        while let Some(stream) = incoming.next().await {
            log::trace!("incoming connection");
            let stream = stream.expect("error while accepting a new client");
            let sender = sender.clone();
            log::trace!("connection ready");
            sender
                .send(Event::Join((next_client_id, stream.clone())))
                .await
                .unwrap_or_else(|e| log::error!("{}", e));
            log::trace!("spawning connection handler: {}", next_client_id);
            ex.spawn(Self::read_client(next_client_id, stream, sender))
                .detach();
            next_client_id += 1;
        }
        log::trace!("stopped to accept incoming connections");
        Ok(())
    }

    pub fn run(&self) -> io::Result<()> {
        let ex = Arc::new(LocalExecutor::new());
        log::trace!("spawning server task");
        future::block_on(ex.run(async {
            let (sender, receiver) = unbounded();
            ex.spawn(Self::serve(ex.clone(), self.session.clone(), sender))
                .detach();
            Self::handle_events(ex.clone(), self.session.to_string(), receiver).await;

            log::info!("no more client, exiting...");
            if let ConnectionMode::Socket(path) = &self.session.mode {
                log::trace!("cleaning session device: {}", path.display());
                fs::remove_file(path).expect("clean session");
                if Session::list().is_empty() {
                    fs::remove_dir(path.parent().unwrap())
                        .unwrap_or_else(|e| log::warn!("could not clean session directory: {}", e));
                }
            }
            Ok(())
        }))
    }
}
