use std::sync::{Arc, RwLock};

use crossbeam_channel as channel;
use failure::Error;

use crate::client::Client;
use crate::jsonrpc::{ClientEvent, Id, Request};
use crate::protocol::{
    notifications,
    requests::{self, Request as _},
    Key, Text,
};
use crate::session::Session;

#[derive(Clone, Debug, Default)]
pub struct Menu {
    pub command: String,
    pub title: String,
    pub search: String,
    pub entries: Vec<notifications::MenuParamsEntry>,
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

#[derive(Debug)]
pub enum ConnectionEvent {
    Echo(Text),
    Info(String, String),
    Menu(Menu),
    Status(notifications::StatusParams),
    View(notifications::ViewParams),
}

#[derive(Clone, Debug, Default)]
pub struct ConnectionState {
    pub client: String,
    pub session: String,
    pub echo: Option<Text>,
    pub status: notifications::StatusParams,
    pub view: notifications::ViewParams,
    pub menu: Option<Menu>,
}

impl ConnectionState {
    fn event_update(&mut self, event: &ClientEvent) -> Option<ConnectionEvent> {
        if let ClientEvent::Notification(notif) = event {
            match notif.method.as_str() {
                "echo" => notif.params::<Text>().ok().unwrap_or(None).map(|text| {
                    self.echo = Some(text.clone());
                    ConnectionEvent::Echo(text)
                }),
                "info" => notif
                    .params::<notifications::InfoParams>()
                    .ok()
                    .unwrap_or(None)
                    .map(|params| {
                        self.client = params.client;
                        self.session = params.session;
                        ConnectionEvent::Info(self.client.to_owned(), self.session.to_owned())
                    }),
                "menu" => notif
                    .params::<notifications::MenuParams>()
                    .ok()
                    .unwrap_or(None)
                    .map(|params| {
                        self.menu = Some(Menu {
                            command: params.command,
                            title: params.title,
                            search: params.search,
                            entries: params.entries,
                            selected: 0,
                        });
                        self.echo = None;
                        ConnectionEvent::Menu(self.menu.clone().unwrap())
                    }),
                "status" => notif
                    .params::<notifications::StatusParams>()
                    .ok()
                    .unwrap_or(None)
                    .map(|params| {
                        self.status = params;
                        ConnectionEvent::Status(self.status.clone())
                    }),
                "view" => notif
                    .params::<notifications::ViewParams>()
                    .ok()
                    .unwrap_or(None)
                    .map(|view| {
                        self.view = view;
                        ConnectionEvent::View(self.view.clone())
                    }),
                _ => None,
            }
        } else {
            None
        }
    }
}

pub struct Connection {
    client: Client,
    state_lock: Arc<RwLock<ConnectionState>>,
    requests: channel::Sender<Request>,
    next_request_id: i32,
}

impl Connection {
    pub fn new(session: &Session) -> Result<Connection, Error> {
        let (client, requests) = Client::new(session)?;
        Ok(Connection {
            client,
            state_lock: Arc::new(RwLock::new(ConnectionState::default())),
            requests,
            next_request_id: 0,
        })
    }

    pub fn state(&self) -> ConnectionState {
        self.state_lock.read().unwrap().clone()
    }

    pub fn connect(&self) -> channel::Receiver<ConnectionEvent> {
        let events = self.client.run();
        let (tx, rx) = channel::unbounded();
        let ctx_lock = self.state_lock.clone();
        std::thread::spawn(move || {
            for ev in events {
                match ev {
                    Ok(e) => {
                        let mut ctx = ctx_lock.write().unwrap();
                        ctx.event_update(&e)
                            .map(|ev| tx.send(ev).expect("send event"));
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
        self.requests.send(message).expect("send request");
    }

    pub fn quit(&mut self) {
        let id = self.request_id();
        self.request(requests::Quit::new_noarg(id));
    }

    pub fn edit(&mut self, file: &str, scratch: bool) {
        let id = self.request_id();
        let params = requests::EditParams {
            file: file.to_owned(),
            path: None,
            scratch,
        };
        self.request(requests::Edit::new(id, params));
    }

    pub fn menu(&mut self, command: &str, search: &str) {
        let id = self.request_id();
        let params = requests::MenuParams {
            command: command.to_owned(),
            search: search.to_owned(),
        };
        self.request(requests::Menu::new(id, params));
    }

    pub fn menu_select(&mut self) {
        if let Some(menu) = self.state().menu {
            let selected = menu.selected_item();
            let id = self.request_id();
            let choice = if selected.is_empty() {
                &menu.search
            } else {
                selected
            };
            let params = requests::MenuSelectParams {
                command: menu.command.to_owned(),
                choice: choice.to_owned(),
            };
            self.request(requests::MenuSelect::new(id, params));
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

    pub fn keys(&mut self, keys: Vec<Key>) {
        let id = self.request_id();
        self.request(requests::Keys::new(id, keys));
    }
}
