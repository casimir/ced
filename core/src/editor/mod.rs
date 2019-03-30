mod buffer;
mod command;
pub mod menu;
mod piece_table;
pub mod view;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;
use std::rc::Rc;

use crossbeam_channel as channel;
use failure::Error;

pub use self::buffer::{Buffer, BufferSource};
use self::command::default_commands;
use self::menu::Menu;
use self::piece_table::PieceTable;
use self::view::{Focus, Lens};
pub use self::view::{View, ViewItem};
use crate::datastruct::StackMap;
use crate::server::BroadcastMessage;
use remote::jsonrpc::{Error as JError, Id, Notification, Request, Response};
use remote::{protocol, response};

pub struct EditorInfo<'a> {
    pub session: &'a str,
    pub cwd: &'a PathBuf,
    pub buffers: &'a [&'a String],
    pub views: &'a [&'a String],
}

#[derive(Clone, Debug, Default)]
pub struct Selection {
    begin: (usize, usize),
    end: (usize, usize),
}

#[derive(Clone, Debug)]
pub struct ClientContext {
    view: Rc<RefCell<View>>,
    selections: HashMap<String, HashMap<String, Selection>>,
}

pub struct Editor {
    session_name: String,
    cwd: PathBuf,
    clients: StackMap<usize, ClientContext>,
    broadcaster: channel::Sender<BroadcastMessage>,
    buffers: HashMap<String, Buffer>,
    views: StackMap<String, Rc<RefCell<View>>>,
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
        editor.views.insert(view.key(), Rc::new(RefCell::new(view)));

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
            protocol::notification::view::new(
                context.view.borrow().to_notification_params(&self.buffers),
            ),
        );
    }

    pub fn add_client(&mut self, id: usize) {
        let context = if let Some(c) = self.clients.latest() {
            self.clients[c].clone()
        } else {
            let latest_view = self.views.latest_value().unwrap();
            let mut selections = HashMap::new();
            selections.insert(
                latest_view.borrow().key(),
                latest_view
                    .borrow()
                    .buffers()
                    .iter()
                    .map(|&b| (b.clone(), Selection::default()))
                    .collect(),
            );
            ClientContext {
                view: Rc::clone(latest_view),
                selections,
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
        if !context.view.borrow().contains_buffer("*debug*") {
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

    fn append_debug(&mut self, content: &str) {
        if let Some(debug_buffer) = self.buffers.get_mut("*debug*") {
            debug_buffer.append(&format!("{}\n", content));
        }
        info!("{}", content);
        for (client_id, context) in self.clients.iter() {
            if context.view.borrow().contains_buffer("*debug*") {
                self.notify_view_update(*client_id);
            }
        }
    }

    fn delete_view(&mut self, view_id: &str) {
        if let Some(view) = self.views.remove(&view_id.to_owned()) {
            self.append_debug(&format!("delete view: {}", view_id));
            for buffer in view.borrow().buffers() {
                let mut has_ref = false;
                for view in self.views.values() {
                    if view.borrow().buffers().iter().any(|&b| b == buffer) {
                        has_ref = true;
                        break;
                    }
                }
                if !has_ref {
                    self.buffers.remove(&buffer.to_owned());
                    self.append_debug(&format!("delete buffer: {}", buffer));
                }
            }
            if self.views.is_empty() {
                if self.buffers.is_empty() {
                    self.open_scratch("*scratch*");
                }
                let view = View::for_buffer("*scratch*");
                self.views.insert(view.key(), Rc::new(RefCell::new(view)));
            }
            let mut to_notify = Vec::new();
            for (id, context) in self.clients.iter_mut() {
                if context.view.borrow().key() == *view_id {
                    context.view = Rc::clone(self.views.latest_value().expect("get latest view"));
                    to_notify.push(*id);
                }
            }
            for id in to_notify {
                self.notify_view_update(id);
            }
        }
    }

    fn modify_view<F>(&mut self, view_id: &str, f: F)
    where
        F: Fn(&mut View),
    {
        let mut new_view = self.views[view_id].borrow().clone();
        let old_key = new_view.key();
        f(&mut new_view);
        let new_key = new_view.key();
        if old_key != new_key {
            if new_view.is_empty() {
                self.delete_view(&old_key);
            } else {
                let view = Rc::new(RefCell::new(new_view));
                self.views.insert(new_key, Rc::clone(&view));
                let mut to_notify = Vec::new();
                for (id, context) in self.clients.iter_mut() {
                    if context.view.borrow().key() == old_key {
                        context.view = Rc::clone(&view);
                        to_notify.push(*id);
                    }
                }
                for id in to_notify {
                    self.notify_view_update(id);
                }
            }
            self.views.remove(&old_key);
        }
    }

    pub fn handle(&mut self, client_id: usize, line: &str) -> Result<Response, Error> {
        let msg: Request = match line.parse() {
            Ok(req) => req,
            Err(err) => {
                error!("{}: {}", err, line);
                return Ok(Response::invalid_request(Id::Null, line));
            }
        };
        trace!("<- ({}) {}", client_id, msg);
        match msg.method.as_str() {
            "edit" => response!(msg, |params| self.command_edit(client_id, params)),
            "quit" => Response::new(msg.id.clone(), self.command_quit(client_id)),
            "view" => response!(msg, |params| self.command_view(client_id, params)),
            "view-delete" => Response::new(msg.id.clone(), self.command_view_delete(client_id)),
            "view-add" => response!(msg, |params| self.command_view_add(client_id, params)),
            "view-remove" => response!(msg, |params| self.command_view_remove(client_id, params)),
            "menu" => response!(msg, |params| self.command_menu(client_id, params)),
            "menu-select" => response!(msg, |params| self.command_menu_select(client_id, params)),
            method => {
                self.append_debug(&format!("unknown command: {}\n", msg));
                Ok(Response::method_not_found(msg.id, method))
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
            // FIXME process diff
            buffer.load_from_disk()
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
            let view = Rc::new(RefCell::new(View::for_buffer(&params.file)));
            self.views.insert(view.borrow().key(), Rc::clone(&view));
            let context = self.clients.get_mut(&client_id).unwrap();
            context.view = view;
        }

        self.append_debug(&format!("edit: {}", params.file));
        if notify_change {
            for (id, ctx) in self.clients.iter() {
                if ctx.view.borrow().contains_buffer(&params.file) {
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
                    context.view = Rc::clone(view);
                }
                self.notify_view_update(client_id);
                Ok(())
            }
            None => {
                if self.buffers.contains_key(&params.view_id) {
                    {
                        let view = Rc::new(RefCell::new(View::for_buffer(&params.view_id)));
                        let key = view.borrow().key();
                        let context = self.clients.get_mut(&client_id).unwrap();
                        context.view = Rc::clone(&view);
                        self.views.entry(key).or_insert(view);
                    }
                    self.notify_view_update(client_id);
                    Ok(())
                } else {
                    Err(JError::invalid_request(&format!(
                        "view does not exist: {}",
                        &params.view_id
                    )))
                }
            }
        }
    }

    pub fn command_view_delete(
        &mut self,
        client_id: usize,
    ) -> Result<protocol::request::view_delete::Result, JError> {
        let view_id = self.clients[&client_id].view.borrow().key();
        self.delete_view(&view_id);
        Ok(())
    }

    pub fn command_view_add(
        &mut self,
        client_id: usize,
        params: &protocol::request::view_add::Params,
    ) -> Result<protocol::request::view_add::Result, JError> {
        if self.buffers.contains_key(&params.buffer) {
            let view_id = self.clients[&client_id].view.borrow().key();
            self.modify_view(&view_id, |view| {
                view.add_lens(Lens {
                    buffer: params.buffer.clone(),
                    focus: Focus::Whole,
                });
            });
            Ok(())
        } else {
            Err(JError::invalid_request(&format!(
                "buffer does not exist: {}",
                &params.buffer
            )))
        }
    }

    pub fn command_view_remove(
        &mut self,
        client_id: usize,
        params: &protocol::request::view_remove::Params,
    ) -> Result<protocol::request::view_remove::Result, JError> {
        if self.buffers.contains_key(&params.buffer) {
            let view_id = self.clients[&client_id].view.borrow().key();
            self.modify_view(&view_id, |view| {
                if view.contains_buffer(&params.buffer) {
                    view.remove_lens_group(&params.buffer);
                }
            });
            Ok(())
        } else {
            Err(JError::invalid_request(&format!(
                "buffer does not exist: {}",
                &params.buffer
            )))
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
        let menu = &self.command_map[&params.command];
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
        let menu = self.command_map.get(&params.command).ok_or_else(|| {
            JError::invalid_params(&format!("unknown command: {}", &params.command))
        })?;
        let mut entry = menu.get(&params.choice);
        if entry.is_none() && menu.is_prompt() {
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
