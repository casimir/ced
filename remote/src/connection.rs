use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crossbeam_channel as channel;
use failure::Error;

use client::Client;
use jsonrpc::{ClientEvent, Id, Request};
use protocol;
use protocol::notification::menu::Entry as MenuEntry;
use session::Session;

#[derive(Clone, Debug, Default)]
pub struct Menu {
    pub command: String,
    pub title: String,
    pub search: String,
    pub entries: Vec<MenuEntry>,
    pub selected: usize,
}

impl Menu {
    fn select_next(&mut self) {
        if self.selected < self.entries.len() - 1 {
            self.selected += 1;
        }
    }

    fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn selected_item(&self) -> &str {
        &self.entries[self.selected].value
    }
}

#[derive(Clone, Debug, Default)]
pub struct ConnectionState {
    pub session: String,
    pub view: protocol::notification::view::Params,
    pub menu: Option<Menu>,
}

impl ConnectionState {
    fn event_update(&mut self, event: &ClientEvent) {
        if let ClientEvent::Notification(notif) = event {
            use protocol::notification::*;
            match notif.method.as_str() {
                "info" => {
                    if let Ok(Some(params)) = notif.params::<info::Params>() {
                        self.session = params.session;
                    }
                }
                "menu" => {
                    if let Ok(Some(params)) = notif.params::<menu::Params>() {
                        self.menu = Some(Menu {
                            command: params.command,
                            title: params.title,
                            search: params.search,
                            entries: params.entries,
                            selected: 0,
                        });
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
                        ctx.event_update(&e);
                        tx.send(e).expect("send event");
                    }
                    Err(e) => error!("{}", e),
                }
            }
        });
        rx
    }

    fn request_id(&mut self) -> Id {
        let id = self.next_request_id;
        self.next_request_id += 1;
        Id::Number(id)
    }

    fn request(&mut self, message: Request) {
        self.pending.insert(message.id.clone(), message.clone());
        self.requests.send(message).expect("send request");
    }

    pub fn command_list(&mut self) {
        let id = self.request_id();
        self.request(protocol::request::command_list::new(id));
    }

    pub fn quit(&mut self) {
        let id = self.request_id();
        self.request(protocol::request::quit::new(id));
    }

    pub fn edit(&mut self, file: &str) {
        let id = self.request_id();
        self.request(protocol::request::edit::new(id, file));
    }

    pub fn menu(&mut self, command: &str, search: &str) {
        let id = self.request_id();
        self.request(protocol::request::menu::new(id, command, search));
    }

    pub fn menu_select(&mut self) {
        if let Some(menu) = self.state().menu {
            let id = self.request_id();
            self.request(protocol::request::menu_select::new(
                id,
                &menu.command,
                menu.selected_item(),
            ));
            self.action_menu_cancel();
        } else {
            warn!("menu_select without active menu");
        }
    }

    pub fn action_menu_select_previous(&mut self) {
        if let Some(ref mut menu) = self.state_lock.write().unwrap().menu {
            menu.select_previous();
        }
    }

    pub fn action_menu_select_next(&mut self) {
        if let Some(ref mut menu) = self.state_lock.write().unwrap().menu {
            menu.select_next();
        }
    }

    pub fn action_menu_cancel(&mut self) {
        self.state_lock.write().unwrap().menu = None;
    }
}
