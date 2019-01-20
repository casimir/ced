mod buffer;
mod command;
pub mod menu;
pub mod view;

use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;

use crossbeam_channel as channel;
use failure::Error;

pub use self::buffer::{Buffer, BufferSource};
use self::command::default_commands;
use self::menu::Menu;
use self::view::{Focus, Lens};
pub use self::view::{View, ViewItem};
use crate::server::BroadcastMessage;
use crate::stackmap::StackMap;
use remote::jsonrpc::{Error as JError, Id, Notification, Request, Response};
use remote::protocol;
use remote::response;

pub struct EditorInfo<'a> {
    pub session: &'a str,
    pub cwd: &'a PathBuf,
    pub buffers: &'a [&'a String],
    pub views: &'a [&'a String],
}

#[derive(Clone, Debug)]
pub struct ClientContext {
    view: View,
    buffer: String,
}

pub struct Editor {
    session_name: String,
    cwd: PathBuf,
    clients: StackMap<usize, ClientContext>,
    broadcaster: channel::Sender<BroadcastMessage>,
    buffers: HashMap<String, Buffer>,
    views: StackMap<String, View>,
    command_map: HashMap<String, Menu>,
    stopped_clients: HashSet<usize>,
}

impl Editor {
    pub fn new(session: &str, broadcaster: channel::Sender<BroadcastMessage>) -> Editor {
        let mut editor = Editor {
            session_name: session.into(),
            cwd: env::current_dir().unwrap_or_else(|_| dirs::home_dir().unwrap_or_default()),
            clients: StackMap::new(),
            broadcaster,
            buffers: HashMap::new(),
            views: StackMap::new(),
            command_map: default_commands(),
            stopped_clients: HashSet::new(),
        };

        let mut view = View::default();
        editor.open_scratch("*debug*");
        editor.append_debug(&format!(
            "command: {}",
            env::args().collect::<Vec<_>>().join(" ")
        ));
        editor.append_debug(&format!("cwd: {}", editor.cwd.display()));
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
        self.broadcaster.send(bm).expect("broadcast message");
    }

    fn broadcast_all(&self, message: Notification) {
        let bm = BroadcastMessage::new(message);
        self.broadcaster.send(bm).expect("broadcast message");
    }

    fn notify(&self, client_id: usize, message: Notification) {
        self.broadcast(message, |&k| k == client_id);
    }

    fn notify_view_update(&self, client_id: usize) {
        let context = &self.clients[&client_id];
        self.notify(
            client_id,
            protocol::notification::view::new(context.view.to_notification_params(&self.buffers)),
        );
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
        self.notify(
            id,
            protocol::notification::info::new(
                id,
                &self.session_name,
                &self.cwd.display().to_string(),
            ),
        );
        if !context.view.contains_buffer("*debug*") {
            self.notify_view_update(id);
        }
    }

    pub fn remove_client(&mut self, id: usize) {
        self.clients.remove(&id);
        self.append_debug(&format!("client left: {}", id));
    }

    pub fn removed_clients(&mut self) -> Vec<usize> {
        let ids: Vec<usize> = self.stopped_clients.iter().cloned().collect();
        self.stopped_clients.clear();
        ids
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
                self.notify_view_update(*client_id);
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
            "edit" => response!(message, |params| self.command_edit(client_id, params)),
            "quit" => Response::new(message.id.clone(), self.command_quit(client_id)),
            "view" => response!(message, |params| self.command_view(client_id, params)),
            "menu" => response!(message, |params| self.command_menu(client_id, params)),
            "menu-select" => response!(message, |params| self
                .command_menu_select(client_id, params)),
            method => {
                self.append_debug(&format!("unknown command: {}\n", message));
                Ok(Response::method_not_found(message.id, method))
            }
        }
        .map_err(Error::from)
    }

    pub fn command_edit(
        &mut self,
        client_id: usize,
        params: &protocol::request::edit::Params,
    ) -> Result<protocol::request::edit::Result, JError> {
        let exists = self.buffers.contains_key(&params.file);
        let notify_change = if params.scratch {
            if !exists {
                self.open_scratch(&params.file);
            }
            false
        } else if exists {
            let buffer = self.buffers.get_mut(&params.file).unwrap();
            buffer.load_from_disk(false)
        } else {
            let path = match params.path.as_ref() {
                Some(path) => PathBuf::from(path),
                None => {
                    let mut absolute = self.cwd.clone();
                    absolute.push(&params.file);
                    absolute
                }
            };
            self.open_file(&params.file, &path);
            true
        };

        {
            let context = self.clients.get_mut(&client_id).unwrap();
            let view = View::for_buffer(&params.file);
            context.view = view.clone();
            self.views.insert(view.key(), view);
        }

        self.append_debug(&format!("edit: {}", params.file));
        if notify_change {
            for (id, ctx) in self.clients.iter() {
                if ctx.view.contains_buffer(&params.file) {
                    self.notify_view_update(*id);
                }
            }
        } else {
            self.notify_view_update(client_id);
        }
        Ok(())
    }

    pub fn command_quit(
        &mut self,
        client_id: usize,
    ) -> Result<protocol::request::quit::Result, JError> {
        self.remove_client(client_id);
        self.stopped_clients.insert(client_id);
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
                self.notify_view_update(client_id);
                Ok(())
            }
            None => {
                if self.buffers.contains_key(&params.view_id) {
                    {
                        let context = self.clients.get_mut(&client_id).unwrap();
                        let view = View::for_buffer(&params.view_id);
                        context.view = self.views.entry(view.key()).or_insert(view).clone();
                    }
                    self.notify_view_update(client_id);
                    Ok(())
                } else {
                    let reason = format!("view does not exist: {}", params.view_id);
                    Err(JError::invalid_request(&reason))
                }
            }
        }
    }

    pub fn command_menu(
        &mut self,
        client_id: usize,
        params: &protocol::request::menu::Params,
    ) -> Result<protocol::request::menu::Result, JError> {
        {
            let menu = self.command_map.get_mut(&params.command).ok_or({
                JError::invalid_params(&format!("unknown command: {}", &params.command))
            })?;
            if params.search.is_empty() {
                let info = EditorInfo {
                    session: &self.session_name,
                    cwd: &self.cwd,
                    buffers: &self.buffers.keys().collect::<Vec<&String>>(),
                    views: &self.views.keys().collect::<Vec<&String>>(),
                };
                menu.populate(&info);
            }
        }
        let menu = self.command_map[&params.command].clone();
        self.notify(
            client_id,
            protocol::notification::menu::new(menu.to_notification_params(&params.search)),
        );
        Ok(())
    }

    pub fn command_menu_select(
        &mut self,
        client_id: usize,
        params: &protocol::request::menu_select::Params,
    ) -> Result<protocol::request::menu_select::Result, JError> {
        let menu = self
            .command_map
            .get(&params.command)
            .ok_or_else(|| {
                JError::invalid_params(&format!("unknown command: {}", &params.command))
            })?
            .clone();
        let mut entry = menu.get(&params.choice);
        if entry.is_none() && menu.has_fake_matches() {
            entry = menu.get("");
        }
        if entry.is_none() {
            return Err(JError::invalid_params(&format!(
                "unknown choice: {}",
                params.choice
            )));
        }
        let action = entry.unwrap().action;
        let res = (action)(&params.choice, self, client_id);
        if let Some(error) = res.err() {
            match error.downcast::<JError>() {
                Ok(err) => return Err(err),
                Err(err) => return Err(JError::internal_error(&err.to_string())),
            }
        }
        Ok(())
    }
}
