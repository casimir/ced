mod buffer;
mod command;
mod core;
mod diff;
pub mod menu;
mod piece_table;
mod range;
mod selection;
pub mod view;

use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;

pub use self::buffer::{Buffer, BufferSource};
use self::command::default_commands;
use self::core::{Core, Notifier};
pub use self::core::{BUFFER_DEBUG, BUFFER_SCRATCH};
use self::menu::Menu;
pub use self::piece_table::Coords;
use self::piece_table::PieceTable;
use self::view::{Focus, Lens};
pub use self::view::{View, ViewItem};
use remote::jsonrpc::{Error, Id, JsonCodingError, Request, Response};
use remote::protocol::{
    notifications::{self, Notification as _},
    requests, KeyEvent,
};
use remote::response;

pub struct EditorInfo<'a> {
    pub session: &'a str,
    pub cwd: &'a PathBuf,
    pub buffers: &'a [String],
    pub views: &'a [String],
}

fn key_to_lua<'a>(lua: rlua::Context<'a>, event: &KeyEvent) -> rlua::Result<rlua::Table<'a>> {
    let table = lua.create_table()?;
    table.set("ctrl", event.ctrl)?;
    table.set("alt", event.alt)?;
    table.set("shift", event.shift)?;
    table.set("value", event.key.to_string())?;
    table.set("display", event.to_string())?;
    Ok(table)
}

struct LuaEditor {
    core: Core,
}

impl LuaEditor {
    fn new(core: Core) -> LuaEditor {
        LuaEditor { core }
    }
}

impl rlua::UserData for LuaEditor {
    fn add_methods<'lua, M: rlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method(
            "set_status_line",
            |_, this, (client, config): (usize, HashMap<String, rlua::Table>)| {
                let mut items = Vec::new();
                for v in config.values() {
                    items.push(notifications::StatusParamsItem {
                        index: v.get("index")?,
                        text: v.get::<_, String>("text")?.into(),
                    })
                }
                items.sort_by(|a, b| a.index.cmp(&b.index));
                this.core
                    .get_notifier()
                    .notify(client, notifications::Status::new(items));
                Ok(())
            },
        );
        methods.add_method(
            "show_hint",
            |_, this, (client, lines): (usize, Vec<String>)| {
                let params = notifications::HintParams {
                    text: lines.iter().map(|l| l.as_str().into()).collect(),
                };
                this.core
                    .get_notifier()
                    .notify(client, notifications::Hint::new(params));
                Ok(())
            },
        );
    }
}

pub struct Editor {
    session_name: String,
    command_map: HashMap<String, Menu>,
    stopped_clients: HashSet<usize>,
    core: Core,
    lua: rlua::Lua,
}

impl Editor {
    pub fn new(session: &str, notifier: impl Into<Notifier>) -> Editor {
        let mut editor = Editor {
            session_name: session.into(),
            command_map: default_commands(),
            stopped_clients: HashSet::new(),
            core: Core::new(notifier.into()),
            lua: rlua::Lua::new(),
        };

        let mut view = View::default();
        editor.core.debug(&format!(
            "command: {}",
            env::args().collect::<Vec<_>>().join(" ")
        ));
        editor
            .core
            .debug(&format!("cwd: {}", editor.cwd().display()));
        view.add_lens(Lens {
            buffer: BUFFER_DEBUG.to_owned(),
            focus: Focus::Whole,
        });
        editor.core.open_scratch(BUFFER_SCRATCH, String::new());
        view.add_lens(Lens {
            buffer: BUFFER_SCRATCH.to_owned(),
            focus: Focus::Whole,
        });
        editor.core.add_view(view);

        let lg_core = editor.core.clone();
        let lg_editor = LuaEditor::new(editor.core.clone());
        editor
            .lua
            .context(|lua: rlua::Context| {
                // TODO handle runtime path correctly
                let rtp = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts");
                lua.load(&format!(
                    "package.path = package.path .. ';{}'",
                    rtp.join("?.lua").display().to_string().replace(r"\", "/")
                ))
                .exec()?;
                lua.globals().set("_CORE", lg_core)?;
                lua.globals().set("_EDITOR", lg_editor)?;
                lua.load("require 'prelude'").exec()
            })
            .expect("load prelude script");

        editor
    }

    pub fn cwd(&self) -> PathBuf {
        self.core.cwd()
    }

    pub fn exec_lua<F, R>(&mut self, source: &str, client_id: usize, f: F) -> rlua::Result<R>
    where
        F: FnOnce(rlua::Context) -> rlua::Result<R>,
    {
        self.lua
            .context(|context| {
                let env_source = format!(
                    r#"
                    env = {{
                        session = "{session}",
                        client = "{client}",
                    }}
                    "#,
                    session = self.session_name,
                    client = client_id,
                );
                context.load(&env_source).exec()?;
                f(context)
            })
            .map_err(|e| {
                let message = e.to_string();
                self.core.debug(&format!(
                    "client {}: exec error: {}\n<<<<<<<\n{}\n>>>>>>>",
                    client_id, source, message
                ));
                self.core.error(client_id, "exec", &message);
                e
            })
    }

    pub fn add_client(&mut self, id: usize) {
        let info = EditorInfo {
            session: &self.session_name,
            cwd: &self.cwd(),
            buffers: &[],
            views: &[],
        };
        self.core.add_client(id, &info);
        let _ = self.exec_lua("add_client", id, |lua| {
            lua.load(&format!("editor:add_client({})", id)).exec()
        });
    }

    pub fn remove_client(&mut self, id: usize) {
        self.core.remove_client(id);
        let _ = self.exec_lua("remove_client", id, |lua| {
            lua.load(&format!("editor:remove_client({})", id)).exec()
        });
    }

    pub fn removed_clients(&mut self) -> Vec<usize> {
        let ids: Vec<usize> = self.stopped_clients.iter().cloned().collect();
        self.stopped_clients.clear();
        ids
    }

    pub fn handle(&mut self, client_id: usize, line: &str) -> Result<Response, JsonCodingError> {
        let msg: Request = match line.parse() {
            Ok(req) => req,
            Err(err) => {
                self.core
                    .error(client_id, "protocol", &format!("{}: {}", err, line));
                return Ok(Response::invalid_request(Id::Null, line));
            }
        };
        log::trace!("<- ({}) {}", client_id, msg);
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
            "exec" => response!(msg, |params| self.command_exec(client_id, params)),
            method => {
                self.core.error(
                    client_id,
                    "protocol",
                    &format!("unknown command: {}\n", msg),
                );
                Ok(Response::method_not_found(msg.id, method))
            }
        }
    }

    pub fn command_edit(
        &mut self,
        client_id: usize,
        params: &<requests::Edit as requests::Request>::Params,
    ) -> Result<<requests::Edit as requests::Request>::Result, Error> {
        self.core.edit(client_id, &params.name, params.scratch);
        Ok(())
    }

    pub fn command_quit(
        &mut self,
        client_id: usize,
    ) -> Result<<requests::Quit as requests::Request>::Result, Error> {
        self.core.remove_client(client_id);
        self.stopped_clients.insert(client_id);
        Ok(())
    }

    pub fn command_view(
        &mut self,
        client_id: usize,
        params: &<requests::View as requests::Request>::Params,
    ) -> Result<<requests::View as requests::Request>::Result, Error> {
        self.core
            .view(client_id, &params)
            .map_err(|e| Error::invalid_request(&e.to_string()))
    }

    pub fn command_view_delete(
        &mut self,
        client_id: usize,
    ) -> Result<<requests::ViewDelete as requests::Request>::Result, Error> {
        self.core.delete_current_view(client_id);
        Ok(())
    }

    pub fn command_view_add(
        &mut self,
        client_id: usize,
        params: &<requests::ViewAdd as requests::Request>::Params,
    ) -> Result<<requests::ViewAdd as requests::Request>::Result, Error> {
        self.core
            .add_to_current_view(client_id, &params)
            .map_err(|e| Error::invalid_request(&e.to_string()))
    }

    pub fn command_view_remove(
        &mut self,
        client_id: usize,
        params: &<requests::ViewRemove as requests::Request>::Params,
    ) -> Result<<requests::ViewRemove as requests::Request>::Result, Error> {
        self.core
            .remove_from_current_view(client_id, &params)
            .map_err(|e| Error::invalid_request(&e.to_string()))
    }

    pub fn command_menu(
        &mut self,
        client_id: usize,
        params: &<requests::Menu as requests::Request>::Params,
    ) -> Result<<requests::Menu as requests::Request>::Result, Error> {
        {
            let cwd = self.cwd();
            let menu = self.command_map.get_mut(&params.command).ok_or({
                Error::invalid_params(&format!("unknown command: {}", &params.command))
            })?;
            if params.search.is_empty() {
                let info = EditorInfo {
                    session: &self.session_name,
                    cwd: &cwd,
                    buffers: &self.core.buffers(),
                    views: &self.core.views(),
                };
                menu.populate(&info);
            }
        }
        let menu = &self.command_map[&params.command];
        self.core.get_notifier().notify(
            client_id,
            notifications::Menu::new(menu.to_notification_params(&params.search)),
        );
        Ok(())
    }

    pub fn command_menu_select(
        &mut self,
        client_id: usize,
        params: &<requests::MenuSelect as requests::Request>::Params,
    ) -> Result<<requests::MenuSelect as requests::Request>::Result, Error> {
        let menu = self.command_map.get(&params.command).ok_or_else(|| {
            Error::invalid_params(&format!("unknown command: {}", &params.command))
        })?;
        let mut entry = menu.get(&params.choice);
        if entry.is_none() && menu.is_prompt() {
            entry = menu.get("");
        }
        if entry.is_none() {
            return Err(Error::invalid_params(&format!(
                "unknown choice: {}",
                params.choice
            )));
        }
        let action = entry.unwrap().action;
        (action)(&params.choice, self, client_id)
    }

    pub fn command_keys(
        &mut self,
        client_id: usize,
        params: &<requests::Keys as requests::Request>::Params,
    ) -> Result<<requests::Keys as requests::Request>::Result, Error> {
        for key in params {
            let result = self.exec_lua("keys", client_id, |lua| {
                let handler = lua
                    .load(&format!("editor.clients[{}].key_handler", client_id))
                    .eval::<rlua::Table>()?;
                let handle_func = handler.get::<_, rlua::Function>("handle")?;
                handle_func.call::<_, ()>((handler, key_to_lua(lua, &key)))
            });
            if let Err(e) = result {
                self.core.error(client_id, "key handler", &e.to_string());
                return Err(Error::internal_error(&e.to_string()));
            }
        }
        Ok(())
    }

    pub fn command_exec(
        &mut self,
        client_id: usize,
        params: &<requests::Exec as requests::Request>::Params,
    ) -> Result<<requests::Exec as requests::Request>::Result, Error> {
        self.exec_lua("exec", client_id, |lua| lua.load(params).exec())
            .map_err(|e| Error::new(1, "exec error".to_string(), e.to_string()).unwrap())
    }
}
