mod buffer;
mod menu;
pub mod view;

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use crossbeam_channel as channel;
use failure::Error;

pub use self::buffer::{Buffer, BufferSource};
use self::menu::Menu;
use self::view::{Focus, Lens};
pub use self::view::{View, ViewItem};
use remote::jsonrpc::{Error as JError, Id, Notification, Request, Response};
use remote::protocol;
use server::BroadcastMessage;
use stackmap::StackMap;

lazy_static! {
    static ref HELP: BTreeMap<&'static str, &'static str> = {
        let mut h = BTreeMap::new();
        h.insert("command-list", "list available commands");
        h.insert("buffer-list", "list open buffers (with content)");
        h.insert(
            "edit <path>",
            "edit a file, reload it from the disk if needed",
        );
        h.insert("view <view_id>", "select an existing view");
        h
    };
}

// FIXME duplicated from jsonrpc
macro_rules! response {
    ($msg:ident, $call:expr) => {
        Response::new(
            $msg.id.clone(),
            match $msg.params() {
                Ok(Some(ref params)) => $call(params),
                Ok(None) => Err(JError::invalid_request("missing field: params")),
                Err(err) => Err(JError::invalid_params(&err.to_string())),
            },
        )
    };
}

#[derive(Clone, Debug)]
pub struct ClientContext {
    view: View,
    buffer: String,
}

pub struct Editor {
    session_name: String,
    clients: StackMap<usize, ClientContext>,
    broadcaster: channel::Sender<BroadcastMessage>,
    buffers: HashMap<String, Buffer>,
    views: StackMap<String, View>,
    menu_cache: Option<Menu>,
}

impl Editor {
    pub fn new(session: &str, broadcaster: channel::Sender<BroadcastMessage>) -> Editor {
        let mut editor = Editor {
            session_name: session.into(),
            clients: StackMap::new(),
            broadcaster,
            buffers: HashMap::new(),
            views: StackMap::new(),
            menu_cache: None,
        };

        let mut view = View::default();
        editor.open_scratch("*debug*");
        view.add_lens(Lens {
            buffer: String::from("*debug*"),
            focus: Focus::Whole,
        });
        editor.open_scratch("*scratch*");
        view.add_lens(Lens {
            buffer: String::from("*scratch*"),
            focus: Focus::Whole,
        });
        editor.views.insert(view.key(), view);

        editor
    }

    fn broadcast<F>(&self, message: Notification, filter: F)
    where
        F: Fn(&usize) -> bool,
    {
        let skiplist = self
            .clients
            .keys()
            .filter(|&k| !filter(k))
            .cloned()
            .collect();
        let bm = BroadcastMessage::new_skip(message, skiplist);
        self.broadcaster.send(bm);
    }

    fn broadcast_all(&self, message: Notification) {
        let bm = BroadcastMessage::new(message);
        self.broadcaster.send(bm);
    }

    fn notify(&self, client_id: usize, message: Notification) {
        self.broadcast(message, |&k| k == client_id);
    }

    pub fn add_client(&mut self, id: usize) {
        let context = if let Some(c) = self.clients.latest() {
            self.clients[c].clone()
        } else {
            ClientContext {
                view: self.views.latest_value().unwrap().clone(),
                buffer: String::new(),
            }
        };
        self.clients.insert(id, context.clone());
        self.append_debug(&format!("new client: {}", id));
        self.notify(id, protocol::notification::info::new(&self.session_name));
        if !context.view.contains_buffer("*debug*") {
            self.notify(
                id,
                protocol::notification::view::new(&context.view, &self.buffers),
            );
        }
    }

    pub fn remove_client(&mut self, id: usize) {
        self.clients.remove(&id);
        self.append_debug(&format!("client left: {}", id));
    }

    fn open_scratch(&mut self, name: &str) {
        let buffer = Buffer::new_scratch(name.to_owned());
        self.buffers.insert(name.into(), buffer);
    }

    fn open_file(&mut self, buffer_name: &str, filename: &PathBuf) {
        let buffer = Buffer::new_file(filename);
        self.buffers.insert(buffer_name.to_string(), buffer);
    }

    fn delete_buffer(&mut self, buffer_name: &str) {
        self.buffers.remove(&buffer_name.to_owned());
        // TODO update view
        if self.buffers.is_empty() {
            self.open_scratch("*scratch*");
        }
    }

    fn append_debug(&mut self, content: &str) {
        if let Some(debug_buffer) = self.buffers.get_mut("*debug*") {
            debug_buffer.append(content);
        }
        info!("{}", content);
        for (client_id, context) in self.clients.iter() {
            if context.view.contains_buffer("*debug*") {
                self.notify(
                    *client_id,
                    protocol::notification::view::new(&context.view, &self.buffers),
                );
            }
        }
    }

    pub fn handle(&mut self, client_id: usize, line: &str) -> Result<Response, Error> {
        let message: Request = match line.parse() {
            Ok(req) => req,
            Err(err) => {
                error!("{}: {}", err, line);
                return Ok(Response::invalid_request(Id::Null, line));
            }
        };
        trace!("<- ({}) {}", client_id, message);
        match message.method.as_str() {
            "command-list" => response!(message, |params| self
                .command_command_list(client_id, params)),
            "edit" => response!(message, |params| self.command_edit(client_id, params)),
            "view" => response!(message, |params| self.command_view(client_id, params)),
            "menu" => response!(message, |params| self.command_menu(client_id, params)),
            "menu-select" => response!(message, |params| self
                .command_menu_select(client_id, params)),
            method => {
                let dm = format!("unknown command: {}\n", message);
                self.append_debug(&dm);
                Ok(Response::method_not_found(message.id, method))
            }
        }.map_err(Error::from)
    }

    fn command_command_list(
        &mut self,
        _client_id: usize,
        _params: &protocol::request::command_list::Params,
    ) -> Result<protocol::request::command_list::Result, JError> {
        Ok(HELP
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect())
    }

    pub fn command_edit(
        &mut self,
        client_id: usize,
        params: &protocol::request::edit::Params,
    ) -> Result<protocol::request::edit::Result, JError> {
        let path = match params.path.as_ref() {
            Some(path) => PathBuf::from(path),
            None => {
                let mut absolute = env::current_dir().unwrap();
                absolute.push(&params.file);
                absolute
            }
        };
        let notify_change = if self.buffers.contains_key(&params.file) {
            let buffer = self.buffers.get_mut(&params.file).unwrap();
            buffer.load_from_disk(false)
        } else {
            self.open_file(&params.file, &path);
            true
        };

        {
            let context = self.clients.get_mut(&client_id).unwrap();
            let mut view = View::default();
            view.add_lens(Lens {
                buffer: params.file.clone(),
                focus: Focus::Whole,
            });
            context.view = view.clone();
            self.views.insert(view.key(), view);
        }

        let dm = format!("edit: {:?}", path);
        self.append_debug(&dm);
        if notify_change {
            for (id, ctx) in self.clients.iter() {
                if ctx.view.contains_buffer(&params.file) {
                    self.notify(
                        *id,
                        protocol::notification::view::new(&ctx.view, &self.buffers),
                    );
                }
            }
        } else {
            let context = &self.clients[&client_id];
            self.notify(
                client_id,
                protocol::notification::view::new(&context.view, &self.buffers),
            );
        }
        Ok(())
    }

    pub fn command_view(
        &mut self,
        client_id: usize,
        params: &protocol::request::view::Params,
    ) -> Result<protocol::request::view::Result, JError> {
        match self.views.get(&params.view_id) {
            Some(view) => {
                {
                    let context = self.clients.get_mut(&client_id).unwrap();
                    context.view = view.clone();
                }
                self.notify(
                    client_id,
                    protocol::notification::view::new(&view, &self.buffers),
                );
                Ok(())
            }
            None => {
                let reason = format!("view does not exist: {}", params.view_id);
                Err(JError::invalid_request(&reason))
            }
        }
    }

    pub fn command_menu(
        &mut self,
        _client_id: usize,
        params: &protocol::request::menu::Params,
    ) -> Result<protocol::request::menu::Result, JError> {
        let in_cache = match self.menu_cache.as_ref() {
            Some(menu) => menu.kind == params.kind,
            None => false,
        };
        self.menu_cache = if in_cache {
            let mut menu = self.menu_cache.take().unwrap();
            menu.filter.search = params.search.to_owned();
            Some(menu)
        } else {
            Some(match params.kind.as_str() {
                "files" => Ok(Menu::files(&params.search)),
                kind => {
                    let reason = &format!("unknown menu kind: {}", kind);
                    Err(JError::invalid_params(reason))
                }
            }?)
        };

        let menu = &self.menu_cache.as_ref().unwrap();
        let entries = menu
            .filtered()
            .iter()
            .filter(|c| c.is_match())
            .map(|c| c.text.clone())
            .collect();
        Ok(protocol::request::menu::Result {
            kind: params.kind.to_owned(),
            title: menu.title.to_owned(),
            search: params.search.to_owned(),
            entries,
        })
    }

    pub fn command_menu_select(
        &mut self,
        client_id: usize,
        params: &protocol::request::menu_select::Params,
    ) -> Result<protocol::request::menu_select::Result, JError> {
        match params.kind.as_str() {
            "files" => {
                let mut path = env::current_dir().unwrap();
                path.push(&params.choice);
                let params = protocol::request::edit::Params {
                    file: params.choice.to_owned(),
                    path: Some(path.into_os_string().into_string().unwrap()),
                };
                self.command_edit(client_id, &params)?;
            }
            _ => {}
        }
        Ok(())
    }
}
