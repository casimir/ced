use std::io;
use std::sync::{Arc, RwLock};

use crate::client::{Client, ClientEventResult, ClientEventStream};
use crate::jsonrpc::{ClientEvent, Id, Request};
use crate::protocol::{
    notifications,
    requests::{self, Request as _},
    KeyEvent, Text,
};
use crate::session::Session;
use async_channel::Sender;
use futures_lite::*;

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
    ConnErr(String),
    Noop,
    Echo(Text),
    Hint(notifications::HintParams),
    Info(String, String),
    Menu(Menu),
    Status(notifications::StatusParams),
    View(notifications::ViewParams),
}

pub type ConnectionEventStream<F> = stream::Map<ClientEventStream, F>;

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
        // TODO check if ConnectionEvent is really useful
        if let ClientEvent::Notification(notif) = event {
            match notif.method.as_str() {
                "echo" => notif.params::<Text>().ok().unwrap_or(None).map(|text| {
                    self.echo = Some(text.clone());
                    ConnectionEvent::Echo(text)
                }),
                "hint" => notif
                    .params::<notifications::HintParams>()
                    .ok()?
                    .map(|params| ConnectionEvent::Hint(params)),
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
    requests: Sender<Request>,
    next_request_id: i32,
}

impl Connection {
    pub fn new(session: Session) -> io::Result<Connection> {
        let (client, requests) = Client::new(session)?;
        Ok(Connection {
            client,
            state_lock: Default::default(),
            requests,
            next_request_id: 0,
        })
    }

    pub fn state(&self) -> ConnectionState {
        self.state_lock.read().unwrap().clone()
    }

    pub async fn connect(
        &self,
    ) -> (
        ConnectionEventStream<impl FnMut(ClientEventResult) -> ConnectionEvent>,
        impl Future<Output = ()>,
    ) {
        let ctx_lock = self.state_lock.clone();
        let (events, request_loop) = self.client.run().await.unwrap();
        (
            events.map(move |ev| match ev {
                Ok(e) => {
                    let mut ctx = ctx_lock.write().unwrap();
                    ctx.event_update(&e).unwrap_or(ConnectionEvent::Noop)
                }
                Err(e) => ConnectionEvent::ConnErr(e.to_string()),
            }),
            request_loop,
        )
    }

    fn request_id(&mut self) -> Id {
        let id = self.next_request_id;
        self.next_request_id += 1;
        Id::Number(id)
    }

    fn request(&mut self, message: Request) {
        future::block_on(self.requests.send(message)).expect("send request");
    }

    pub fn quit(&mut self) {
        let id = self.request_id();
        self.request(requests::Quit::new_noarg(id));
    }

    pub fn edit(&mut self, name: String, scratch: bool) {
        let id = self.request_id();
        let params = requests::EditParams { name, scratch };
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
            log::warn!("menu_select without active menu");
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

    pub fn keys<K>(&mut self, keys: K)
    where
        K: Into<Vec<KeyEvent>>,
    {
        let id = self.request_id();
        self.request(requests::Keys::new(id, keys));
    }

    pub fn exec(&mut self, source: &str) {
        let id = self.request_id();
        self.request(requests::Exec::new(id, source.to_owned()));
    }
}
