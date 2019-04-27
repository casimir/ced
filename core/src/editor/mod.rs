mod buffer;
mod command;
mod core;
mod diff;
pub mod menu;
mod piece_table;
mod range;
pub mod view;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;
use std::rc::Rc;

use crossbeam_channel as channel;
use failure::Error;
use rlua;

pub use self::buffer::{Buffer, BufferSource};
use self::command::default_commands;
use self::core::Core;
use self::menu::Menu;
use self::piece_table::PieceTable;
use self::range::Range;
use self::view::{Focus, Lens};
pub use self::view::{View, ViewItem};
use crate::server::BroadcastMessage;
use remote::jsonrpc::{Error as JError, Id, Notification, Request, Response};
use remote::protocol::notification::view::Params as NotificationParams;
use remote::{protocol, response};

pub struct EditorInfo<'a> {
    pub session: &'a str,
    pub cwd: &'a PathBuf,
    pub buffers: &'a [String],
    pub views: &'a [String],
}

#[derive(Clone, Debug)]
pub struct ClientContext {
    view: Rc<RefCell<View>>,
    selections: HashMap<String, HashMap<String, Range>>,
}

#[derive(Clone)]
pub struct Notifier {
    sender: channel::Sender<BroadcastMessage>,
}

impl Notifier {
    pub fn broadcast<I>(&self, message: Notification, only_clients: I)
    where
        I: Into<Option<Vec<usize>>>,
    {
        let bm = match only_clients.into() {
            Some(cs) => BroadcastMessage::for_clients(cs, message),
            None => BroadcastMessage::new(message),
        };
        self.sender.send(bm).expect("broadcast message");
    }

    pub fn notify(&self, client_id: usize, message: Notification) {
        self.broadcast(message, vec![client_id]);
    }

    pub fn info_update(&self, client_id: usize, info: &EditorInfo) {
        self.notify(
            client_id,
            protocol::notification::info::new(
                client_id,
                &info.session,
                &info.cwd.display().to_string(),
            ),
        );
    }

    pub fn view_update(&self, params: Vec<(usize, NotificationParams)>) {
        for (client_id, np) in params {
            self.notify(client_id, protocol::notification::view::new(np));
        }
    }
}

pub struct Editor {
    session_name: String,
    cwd: PathBuf,
    command_map: HashMap<String, Menu>,
    stopped_clients: HashSet<usize>,
    core: Core,
    lua: rlua::Lua,
}

impl Editor {
    pub fn new(session: &str, broadcaster: channel::Sender<BroadcastMessage>) -> Editor {
        let notifier = Notifier {
            sender: broadcaster,
        };
        let mut editor = Editor {
            session_name: session.into(),
            cwd: env::current_dir().unwrap_or_else(|_| dirs::home_dir().unwrap_or_default()),
            command_map: default_commands(),
            stopped_clients: HashSet::new(),
            core: Core::new(notifier),
            lua: rlua::Lua::new(),
        };

        let mut view = View::default();
        editor.core.open_scratch("*debug*");
        editor.core.append_debug(&format!(
            "command: {}",
            env::args().collect::<Vec<_>>().join(" ")
        ));
        editor
            .core
            .append_debug(&format!("cwd: {}", editor.cwd.display()));
        view.add_lens(Lens {
            buffer: String::from("*debug*"),
            focus: Focus::Whole,
        });
        editor.core.open_scratch("*scratch*");
        view.add_lens(Lens {
            buffer: String::from("*scratch*"),
            focus: Focus::Whole,
        });
        editor.core.add_view(view);

        let lua_pipe = editor.core.clone();
        editor
            .lua
            .context(|lua: rlua::Context| {
                lua.globals().set("editor", lua_pipe)?;
                lua.load(include_str!("../../scripts/prelude.lua")).exec()
            })
            .expect("load prelude script");

        editor
    }

    pub fn add_client(&mut self, id: usize) {
        let info = EditorInfo {
            session: &self.session_name,
            cwd: &self.cwd,
            buffers: &[],
            views: &[],
        };
        self.core.add_client(id, &info);
    }

    pub fn remove_client(&mut self, id: usize) {
        self.core.remove_client(id);
    }

    pub fn removed_clients(&mut self) -> Vec<usize> {
        let ids: Vec<usize> = self.stopped_clients.iter().cloned().collect();
        self.stopped_clients.clear();
        ids
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
            "keys" => response!(msg, |params| self.command_keys(client_id, params)),
            method => {
                self.core
                    .append_debug(&format!("unknown command: {}\n", msg));
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
        if params.scratch {
            self.core
                .edit(client_id, &params.file, None, params.scratch);
        } else {
            let path = match params.path.as_ref() {
                Some(path) => PathBuf::from(path),
                None => {
                    let mut absolute = self.cwd.clone();
                    absolute.push(&params.file);
                    absolute
                }
            };
            self.core
                .edit(client_id, &params.file, Some(&path), params.scratch);
        }
        Ok(())
    }

    pub fn command_quit(
        &mut self,
        client_id: usize,
    ) -> Result<protocol::request::quit::Result, JError> {
        self.core.remove_client(client_id);
        self.stopped_clients.insert(client_id);
        Ok(())
    }

    pub fn command_view(
        &mut self,
        client_id: usize,
        params: &protocol::request::view::Params,
    ) -> Result<protocol::request::view::Result, JError> {
        self.core
            .view(client_id, &params.view_id)
            .map_err(|e| JError::invalid_request(&e.to_string()))
    }

    pub fn command_view_delete(
        &mut self,
        client_id: usize,
    ) -> Result<protocol::request::view_delete::Result, JError> {
        self.core.delete_current_view(client_id);
        Ok(())
    }

    pub fn command_view_add(
        &mut self,
        client_id: usize,
        params: &protocol::request::view_add::Params,
    ) -> Result<protocol::request::view_add::Result, JError> {
        self.core
            .add_to_current_view(client_id, &params.buffer)
            .map_err(|e| JError::invalid_request(&e.to_string()))
    }

    pub fn command_view_remove(
        &mut self,
        client_id: usize,
        params: &protocol::request::view_remove::Params,
    ) -> Result<protocol::request::view_remove::Result, JError> {
        self.core
            .remove_from_current_view(client_id, &params.buffer)
            .map_err(|e| JError::invalid_request(&e.to_string()))
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
                    buffers: &self.core.buffers(),
                    views: &self.core.views(),
                };
                menu.populate(&info);
            }
        }
        let menu = &self.command_map[&params.command];
        self.core.get_notifier().notify(
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

    pub fn command_keys(
        &mut self,
        client_id: usize,
        params: &protocol::request::keys::Params,
    ) -> Result<protocol::request::keys::Result, JError> {
        self.lua
            .context(|lua: rlua::Context| {
                let globals = lua.globals();
                let handler: rlua::Function = globals.get("key_handler")?;
                for key in &params.keys {
                    handler.call((client_id, key.as_str()))?;
                }
                Ok(())
            })
            .map_err(|e: rlua::Error| JError::internal_error(&e.to_string()))?;
        Ok(())
    }
}
