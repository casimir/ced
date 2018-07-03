pub mod buffer;

use std::collections::BTreeMap;
use std::path::PathBuf;

use jsonrpc_lite::{Error as JRError, JsonRpc, Params};
use serde_json::{self, Map, Value};

use self::buffer::{Buffer, BufferSource};
use stackmap::StackMap;

lazy_static! {
    static ref HELP: BTreeMap<&'static str, &'static str> = {
        let mut h = BTreeMap::new();
        h.insert("command-list", "list available commands");
        h.insert("edit", "edit a file, reload it from the disk if needed");
        h.insert("buffer-list", "list open buffers (with content)");
        h.insert("buffer-select", "select a buffer by its name");
        h.insert("buffer-delete", "delete a buffer by its name");
        h
    };
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Protocol(err: serde_json::Error) {
            from()
            display("protocol error: {}", err)
        }
    }
}

type Result<T> = ::std::result::Result<T, Error>;

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
                editor.open_file(&filename.into());
            }
        }
        editor
    }

    fn notification_init(&self, client_id: usize) -> JsonRpc {
        let context = &self.clients[&client_id];
        let mut params: Map<String, Value> = Map::new();
        params.insert("session".into(), self.session_name.clone().into());
        params.insert(
            "buffer_list".into(),
            self.buffers
                .values()
                .map(|b| self.get_buffer_value(&b))
                .collect::<Vec<Value>>()
                .into(),
        );
        params.insert("buffer_current".into(), context.buffer.clone().into());
        JsonRpc::notification_with_params("init", params)
    }

    pub fn add_client(&mut self, id: usize) -> Result<(JsonRpc, Option<JsonRpc>)> {
        let context = if let Some(c) = self.clients.latest() {
            self.clients[c].clone()
        } else {
            ClientContext {
                buffer: self.buffers.latest_value().unwrap().absolute_name(),
            }
        };
        self.clients.insert(id, context);
        Ok((self.notification_init(id), None))
    }

    pub fn remove_client(&mut self, id: usize) -> Result<Option<JsonRpc>> {
        self.clients.remove(&id);
        Ok(None)
    }

    fn open_scratch(&mut self, name: &str) {
        let buffer = Buffer::new_scratch(name.to_owned());
        self.buffers.insert(buffer.absolute_name(), buffer);
    }

    fn open_file(&mut self, filename: &PathBuf) {
        let buffer = Buffer::new_file(filename);
        self.buffers.insert(buffer.absolute_name(), buffer);
    }

    fn delete_buffer(&mut self, buffer_name: &String) {
        self.buffers.remove(buffer_name);
        if self.buffers.is_empty() {
            self.open_scratch("*scratch*");
        }
    }

    fn get_buffer_value(&self, buffer: &Buffer) -> Value {
        let buffer_sources = self.buffers
            .values()
            .map(|b| b.source.clone())
            .collect::<Vec<BufferSource>>();

        let mut val: Map<String, Value> = Map::new();
        val.insert("name".into(), buffer.absolute_name().into());
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

    fn notification_buffer_changed(&mut self) -> JsonRpc {
        JsonRpc::notification_with_params(
            "buffer_changed",
            self.get_buffer_value(&self.buffers.latest_value().unwrap()),
        )
    }

    pub fn handle(&mut self, client_id: usize, line: &str) -> Result<(JsonRpc, Option<JsonRpc>)> {
        trace!("<- ({}) {:?}", client_id, line);
        let message = JsonRpc::parse(line)?;
        let to_write = match message.get_method() {
            Some(name) => match name {
                "command-list" => self.command_list(client_id, &message),
                "edit" => self.command_edit(client_id, &message),
                "buffer-list" => self.command_buffer_list(client_id, &message),
                "buffer-select" => self.command_buffer_select(client_id, &message),
                "buffer-delete" => self.command_buffer_delete(client_id, &message),
                _ => {
                    let req_id = message.get_id().unwrap();
                    (JsonRpc::error(req_id, JRError::method_not_found()), None)
                }
            },
            _ => {
                let dm = format!("unknown command: {:?}\n", message);
                self.append_debug(&dm);
                let req_id = message.get_id().unwrap();
                (JsonRpc::error(req_id, JRError::method_not_found()), None)
            }
        };
        Ok(to_write)
    }

    fn handle_edit(&mut self, client_id: usize, message: &JsonRpc) -> JsonRpc {
        let req_id = message.get_id().unwrap();
        let file_path = match message.get_params().unwrap() {
            Params::Array(args) => args[0].as_str().unwrap().to_owned(),
            _ => String::new(),
        };
        let path = PathBuf::from(file_path);
        let dm = format!("edit: {:?}", path);
        self.append_debug(&dm);
        let path_str = path.clone().into_os_string().into_string().unwrap();
        self.buffers
            .set_last(path_str.clone())
            .unwrap_or_else(|_| self.open_file(&path));
        {
            let buffer = self.buffers.get_mut(&path_str).unwrap();
            buffer.load_from_disk(false);
        }
        {
            let context = self.clients.get_mut(&client_id).unwrap();
            context.buffer = self.buffers.latest().unwrap().to_string();
        }
        JsonRpc::success(
            req_id,
            &self.get_buffer_value(self.buffers.latest_value().unwrap()),
        )
    }

    fn handle_list_buffer(&self, message: &JsonRpc) -> JsonRpc {
        let req_id = message.get_id().unwrap();
        let params = self.buffers
            .values()
            .map(|b| self.get_buffer_value(&b))
            .collect::<Vec<Value>>();
        JsonRpc::success(req_id, &params.into())
    }

    fn handle_select_buffer(&mut self, client_id: usize, message: &JsonRpc) -> JsonRpc {
        let req_id = message.get_id().unwrap();
        let buffer_name = match message.get_params().unwrap() {
            Params::Array(args) => args[0].as_str().unwrap().to_owned(),
            _ => String::new(),
        };
        let dm = format!("buffer-select: {}\n", buffer_name);
        self.append_debug(&dm);
        if self.buffers.contains_key(&buffer_name) {
            {
                self.buffers.set_last(buffer_name.clone()).unwrap();
                let buffer = self.buffers.get_mut(&buffer_name).unwrap();
                buffer.load_from_disk(false);
            }
            {
                let context = self.clients.get_mut(&client_id).unwrap();
                context.buffer = self.buffers.latest().unwrap().to_string();
            }
            JsonRpc::success(
                req_id,
                &self.get_buffer_value(&self.buffers.latest_value().unwrap()),
            )
        } else {
            let mut error = JRError::invalid_params();
            let details = format!("buffer '{}' does not exist", buffer_name);
            error.data = Some(details.to_string().into());
            JsonRpc::error(req_id, error)
        }
    }

    fn handle_delete_buffer(
        &mut self,
        client_id: usize,
        message: &JsonRpc,
    ) -> (JsonRpc, Option<JsonRpc>) {
        let req_id = message.get_id().unwrap();
        let buffer_name = match message.get_params().unwrap() {
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
                JsonRpc::success(req_id, &params.into()),
                Some(self.notification_buffer_changed()),
            )
        } else {
            let mut error = JRError::invalid_params();
            let details = format!("buffer '{}' does not exist", buffer_name);
            error.data = Some(details.to_string().into());
            (JsonRpc::error(req_id, error), None)
        }
    }

    fn command_list(&mut self, _client_id: usize, message: &JsonRpc) -> (JsonRpc, Option<JsonRpc>) {
        let req_id = message.get_id().unwrap();
        let mut result = Map::new();
        for (cmd, help) in HELP.iter() {
            result.insert(cmd.to_string(), help.to_string().into());
        }
        (JsonRpc::success(req_id, &result.into()), None)
    }

    fn command_edit(&mut self, client_id: usize, message: &JsonRpc) -> (JsonRpc, Option<JsonRpc>) {
        (self.handle_edit(client_id, &message), None)
    }

    fn command_buffer_list(
        &mut self,
        _client_id: usize,
        message: &JsonRpc,
    ) -> (JsonRpc, Option<JsonRpc>) {
        (self.handle_list_buffer(&message), None)
    }

    fn command_buffer_select(
        &mut self,
        client_id: usize,
        message: &JsonRpc,
    ) -> (JsonRpc, Option<JsonRpc>) {
        (self.handle_select_buffer(client_id, &message), None)
    }

    fn command_buffer_delete(
        &mut self,
        client_id: usize,
        message: &JsonRpc,
    ) -> (JsonRpc, Option<JsonRpc>) {
        self.handle_delete_buffer(client_id, &message)
    }
}
