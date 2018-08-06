pub mod buffer;

use std::collections::BTreeMap;
use std::path::PathBuf;

use failure::Error;
use jsonrpc_lite::{Error as JRError, JsonRpc, Params};
use serde_json::{Map, Value};

use self::buffer::{Buffer, BufferSource};
use remote::protocol::Object;
use stackmap::StackMap;

lazy_static! {
    static ref HELP: BTreeMap<&'static str, &'static str> = {
        let mut h = BTreeMap::new();
        h.insert("command-list", "list available commands");
        h.insert(
            "edit <path>",
            "edit a file, reload it from the disk if needed",
        );
        h.insert("buffer-list", "list open buffers (with content)");
        h.insert("buffer-select <buffer>", "select a buffer by its name");
        h.insert("buffer-delete <buffer>", "delete a buffer by its name");
        h
    };
}

#[derive(Clone, Debug)]
pub struct ClientContext {
    buffer: String,
}

pub struct Editor {
    session_name: String,
    clients: StackMap<usize, ClientContext>,
    buffers: StackMap<String, Buffer>,
}

impl Editor {
    pub fn new(session: &str, filenames: &[&str]) -> Editor {
        let mut editor = Editor {
            session_name: session.into(),
            clients: StackMap::new(),
            buffers: StackMap::new(),
        };
        editor.open_scratch("*debug*");
        if filenames.is_empty() {
            editor.open_scratch("*scratch*");
        } else {
            for filename in filenames {
                editor.open_file(filename, &filename.into());
            }
        }
        editor
    }

    fn notification_init(&self, client_id: usize) -> Object {
        let context = &self.clients[&client_id];
        let mut params: Map<String, Value> = Map::new();
        params.insert("session".into(), self.session_name.clone().into());
        params.insert(
            "buffer_list".into(),
            self.buffers
                .iter()
                .map(|(n, b)| self.get_buffer_value(n, b))
                .collect::<Vec<Value>>()
                .into(),
        );
        params.insert("buffer_current".into(), context.buffer.clone().into());
        JsonRpc::notification_with_params("init", params).into()
    }

    pub fn add_client(&mut self, id: usize) -> Result<(Object, Option<Object>), Error> {
        let context = if let Some(c) = self.clients.latest() {
            self.clients[c].clone()
        } else {
            ClientContext {
                buffer: self.buffers.latest().unwrap().clone(),
            }
        };
        self.clients.insert(id, context);
        Ok((self.notification_init(id), None))
    }

    pub fn remove_client(&mut self, id: usize) -> Result<Option<Object>, Error> {
        self.clients.remove(&id);
        Ok(None)
    }

    fn open_scratch(&mut self, name: &str) {
        let buffer = Buffer::new_scratch(name.to_owned());
        self.buffers.insert(name.into(), buffer);
    }

    fn open_file(&mut self, buffer_name: &str, filename: &PathBuf) {
        let buffer = Buffer::new_file(filename);
        self.buffers.insert(buffer_name.to_string(), buffer);
    }

    fn delete_buffer(&mut self, buffer_name: &String) {
        self.buffers.remove(buffer_name);
        if self.buffers.is_empty() {
            self.open_scratch("*scratch*");
        }
    }

    fn get_buffer_value(&self, name: &str, buffer: &Buffer) -> Value {
        let buffer_sources = self
            .buffers
            .values()
            .map(|b| b.source.clone())
            .collect::<Vec<BufferSource>>();

        let mut val: Map<String, Value> = Map::new();
        val.insert("name".into(), name.into());
        val.insert("label".into(), buffer.shortest_name(&buffer_sources).into());
        val.insert("content".into(), buffer.to_string().into());
        val.into()
    }

    fn append_debug(&mut self, content: &str) {
        if let Some(debug_buffer) = self.buffers.get_mut("*debug*") {
            debug_buffer.append(content);
        }
        info!("{}", content);
    }

    fn notification_buffer_changed(&mut self) -> Object {
        let name = self.buffers.latest().unwrap();
        JsonRpc::notification_with_params(
            "buffer-changed",
            self.get_buffer_value(name, &self.buffers[name]),
        ).into()
    }

    pub fn handle(
        &mut self,
        client_id: usize,
        line: &str,
    ) -> Result<(Object, Option<Object>), Error> {
        trace!("<- ({}) {:?}", client_id, line);
        let message: Object = line.parse()?;
        let to_write = match message.inner().get_method() {
            Some(name) => match name {
                "buffer-list" => self.command_buffer_list(client_id, &message),
                "buffer-select" => self.command_buffer_select(client_id, &message),
                "buffer-delete" => self.command_buffer_delete(client_id, &message),
                "command-list" => self.command_list(client_id, &message),
                "edit" => self.command_edit(client_id, &message),
                _ => match message.inner().get_id() {
                    Some(req_id) => (
                        JsonRpc::error(req_id, JRError::method_not_found()).into(),
                        None,
                    ),
                    None => (JsonRpc::error((), JRError::invalid_request()).into(), None),
                },
            },
            _ => {
                let dm = format!("unknown command: {}\n", message);
                self.append_debug(&dm);
                match message.inner().get_id() {
                    Some(req_id) => (
                        JsonRpc::error(req_id, JRError::method_not_found()).into(),
                        None,
                    ),
                    None => (JsonRpc::error((), JRError::invalid_request()).into(), None),
                }
            }
        };
        Ok(to_write)
    }

    fn command_buffer_list(
        &mut self,
        _client_id: usize,
        message: &Object,
    ) -> (Object, Option<Object>) {
        if let Some(req_id) = message.inner().get_id() {
            let params = self
                .buffers
                .iter()
                .map(|(n, b)| self.get_buffer_value(n, b))
                .collect::<Vec<Value>>();
            (JsonRpc::success(req_id, &params.into()).into(), None)
        } else {
            (JsonRpc::error((), JRError::invalid_request()).into(), None)
        }
    }

    fn command_buffer_select(
        &mut self,
        client_id: usize,
        message: &Object,
    ) -> (Object, Option<Object>) {
        if let Some(req_id) = message.inner().get_id() {
            let buffer_name = match message.inner().get_params().unwrap() {
                Params::Array(args) => args[0].as_str().unwrap().to_owned(),
                _ => String::new(),
            };
            let dm = format!("buffer-select: {}", buffer_name);
            self.append_debug(&dm);
            if self.buffers.contains_key(&buffer_name) {
                self.buffers.set_last(buffer_name.clone()).unwrap();
                {
                    let context = self.clients.get_mut(&client_id).unwrap();
                    context.buffer = self.buffers.latest().unwrap().to_string();
                }
                (JsonRpc::success(req_id, &buffer_name.into()).into(), None)
            } else {
                let mut error = JRError::invalid_params();
                let details = format!("buffer '{}' does not exist", buffer_name);
                error.data = Some(details.to_string().into());
                (JsonRpc::error(req_id, error).into(), None)
            }
        } else {
            (JsonRpc::error((), JRError::invalid_request()).into(), None)
        }
    }

    fn command_buffer_delete(
        &mut self,
        client_id: usize,
        message: &Object,
    ) -> (Object, Option<Object>) {
        if let Some(req_id) = message.inner().get_id() {
            let buffer_name = match message.inner().get_params().unwrap() {
                Params::Array(args) => args[0].as_str().unwrap().to_owned(),
                _ => String::new(),
            };
            let dm = format!("buffer-delete: {}", buffer_name);
            self.append_debug(&dm);
            if self.buffers.contains_key(&buffer_name) {
                self.delete_buffer(&buffer_name);
                {
                    let context = self.clients.get_mut(&client_id).unwrap();
                    context.buffer = self.buffers.latest().unwrap().to_string();
                }
                let mut params: Map<String, Value> = Map::new();
                params.insert("buffer_deleted".into(), buffer_name.into());
                (
                    JsonRpc::success(req_id, &params.into()).into(),
                    Some(self.notification_buffer_changed()),
                )
            } else {
                let mut error = JRError::invalid_params();
                let details = format!("buffer '{}' does not exist", buffer_name);
                error.data = Some(details.to_string().into());
                (JsonRpc::error(req_id, error).into(), None)
            }
        } else {
            (JsonRpc::error((), JRError::invalid_request()).into(), None)
        }
    }

    fn command_list(&mut self, _client_id: usize, message: &Object) -> (Object, Option<Object>) {
        if let Some(req_id) = message.inner().get_id() {
            let mut result = Map::new();
            for (cmd, help) in HELP.iter() {
                result.insert(cmd.to_string(), help.to_string().into());
            }
            (JsonRpc::success(req_id, &result.into()).into(), None)
        } else {
            (JsonRpc::error((), JRError::invalid_request()).into(), None)
        }
    }

    fn command_edit(&mut self, client_id: usize, message: &Object) -> (Object, Option<Object>) {
        if let Some(req_id) = message.inner().get_id() {
            let file_path = match message.inner().get_params().unwrap() {
                Params::Array(args) => args[0].as_str().unwrap().to_owned(),
                _ => String::new(),
            };
            let path = PathBuf::from(&file_path);
            let mut notify_change = false;
            let dm = format!("edit: {:?}", path);
            self.append_debug(&dm);
            self.buffers
                .set_last(file_path.clone())
                .unwrap_or_else(|_| {
                    self.open_file(&file_path, &path);
                    notify_change = true
                });
            {
                let buffer = self.buffers.get_mut(&file_path).unwrap();
                notify_change |= buffer.load_from_disk(false);
            }
            {
                let context = self.clients.get_mut(&client_id).unwrap();
                context.buffer = self.buffers.latest().unwrap().to_string();
            }
            (
                JsonRpc::success(req_id, &file_path.into()).into(),
                if notify_change {
                    Some(self.notification_buffer_changed())
                } else {
                    None
                },
            )
        } else {
            (JsonRpc::error((), JRError::invalid_request()).into(), None)
        }
    }
}
