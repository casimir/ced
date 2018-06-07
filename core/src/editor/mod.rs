pub mod buffer;
pub mod cmd;

use std::collections::{BTreeMap, HashMap};
use std::ops::{Index, IndexMut};
use std::path::PathBuf;

use jsonrpc_lite::{Error as JRError, JsonRpc, Params};
use serde_json::{self, Map, Value};

use self::buffer::{find_shortest_name, Buffer, BufferSource};
use self::cmd::Command;

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

struct ClientContext {}

pub struct Editor<'a> {
    clients: HashMap<usize, ClientContext>,
    commands: BTreeMap<&'a str, Command<'a>>,
    buffer_list: Vec<Buffer>,
    buffer_selected_idx: usize,
}

impl<'a> Editor<'a> {
    pub fn new(filenames: Vec<&str>) -> Editor {
        let mut editor = Editor {
            clients: HashMap::new(),
            commands: BTreeMap::new(),
            buffer_list: Vec::new(),
            buffer_selected_idx: 0,
        };
        for cmd in &cmd::COMMAND_MAP {
            editor.commands.insert(cmd.name, *cmd);
        }
        editor.open_scratch("*debug*");
        if filenames.is_empty() {
            editor.open_scratch("*scratch*");
        } else {
            for filename in &filenames {
                editor.open_file(PathBuf::from(filename));
            }
        }
        editor
    }

    fn notification_init(&self) -> JsonRpc {
        let mut params: Map<String, Value> = Map::new();
        params.insert(
            String::from("buffer_list"),
            Value::from(
                self.buffer_list
                    .iter()
                    .enumerate()
                    .map(|(i, _)| self.get_buffer_value(i))
                    .collect::<Vec<Value>>(),
            ),
        );
        let buffer_current_name = self.get_buffer_name(self.buffer_selected_idx);
        params.insert(
            String::from("buffer_current"),
            Value::from(buffer_current_name),
        );
        JsonRpc::notification_with_params("init", params)
    }

    pub fn add_client(&mut self, id: usize) -> Result<(JsonRpc, Option<JsonRpc>)> {
        self.clients.insert(id, ClientContext {});
        Ok((self.notification_init(), None))
    }

    pub fn remove_client(&mut self, id: usize) -> Result<Option<JsonRpc>> {
        self.clients.remove(&id);
        Ok(None)
    }

    fn open_scratch(&mut self, name: &str) {
        let buffer = Buffer::new_scratch(name.to_owned());
        self.buffer_list.push(buffer);
        self.buffer_selected_idx = self.buffer_list.len() - 1;
    }

    fn open_file(&mut self, filename: PathBuf) {
        let buffer = Buffer::new_file(filename);
        self.buffer_list.push(buffer);
        self.buffer_selected_idx = self.buffer_list.len() - 1;
    }

    fn delete_buffer(&mut self, idx: usize) {
        self.buffer_list.remove(idx);
        if self.buffer_list.is_empty() {
            self.open_scratch("*scratch*");
        }
        if self.buffer_selected_idx != 0 {
            self.buffer_selected_idx -= 1;
        }
    }

    fn get_buffer_name(&self, idx: usize) -> String {
        let buffer_sources = self.buffer_list.iter().map(|x| &x.source).collect();
        find_shortest_name(&buffer_sources, idx)
    }

    fn get_buffer_value(&self, idx: usize) -> Value {
        let mut val: Map<String, Value> = Map::new();
        val.insert(String::from("name"), Value::from(self.get_buffer_name(idx)));
        val.insert(
            String::from("content"),
            Value::from(self.buffer_list.index(idx).to_string()),
        );
        Value::from(val)
    }

    fn get_buffer_idx(&self, buffer_name: &str) -> Option<usize> {
        self.buffer_list
            .iter()
            .enumerate()
            .map(|(i, _)| self.get_buffer_name(i))
            .position(|x| x == buffer_name)
    }

    fn get_buffer_idx_from_path(&self, path: &PathBuf) -> Option<usize> {
        self.buffer_list
            .iter()
            .position(|x| x.source == BufferSource::File(path.clone()))
    }

    fn append_debug(&mut self, content: &str) {
        let debug_buffer = self.buffer_list.index_mut(0);
        debug_buffer.append(content);
        eprintln!("{}", content);
    }

    fn notification_buffer_changed(&mut self) -> JsonRpc {
        let params = self.get_buffer_value(self.buffer_selected_idx);
        JsonRpc::notification_with_params("buffer_changed", params)
    }

    pub fn handle(&mut self, client_id: usize, line: &str) -> Result<(JsonRpc, Option<JsonRpc>)> {
        let message = JsonRpc::parse(line)?;
        let to_write = match message.get_method() {
            Some(name) => match self.commands.clone().get(name) {
                Some(cmd) => (cmd.exec)(self, &message),
                None => {
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

    fn handle_edit(&mut self, message: &JsonRpc) -> JsonRpc {
        let req_id = message.get_id().unwrap();
        let file_path = match message.get_params().unwrap() {
            Params::Array(args) => args[0].as_str().unwrap().to_owned(),
            _ => String::new(),
        };
        let path = PathBuf::from(file_path);
        let dm = format!("edit: {:?}", path);
        self.append_debug(&dm);
        match self.get_buffer_idx_from_path(&path) {
            Some(idx) => self.buffer_selected_idx = idx,
            None => self.open_file(path),
        };
        {
            let curbuf = &mut self.buffer_list[self.buffer_selected_idx];
            curbuf.load_from_disk(false);
        }
        JsonRpc::success(req_id, &self.get_buffer_value(self.buffer_selected_idx))
    }

    fn handle_list_buffer(&self, message: &JsonRpc) -> JsonRpc {
        let req_id = message.get_id().unwrap();
        let params = self
            .buffer_list
            .iter()
            .enumerate()
            .map(|(i, _)| self.get_buffer_value(i))
            .collect::<Vec<Value>>();
        JsonRpc::success(req_id, &Value::from(params))
    }

    fn handle_select_buffer(&mut self, message: &JsonRpc) -> JsonRpc {
        let req_id = message.get_id().unwrap();
        let buffer_name = match message.get_params().unwrap() {
            Params::Array(args) => args[0].as_str().unwrap().to_owned(),
            _ => String::new(),
        };
        let dm = format!("buffer-select: {}\n", buffer_name);
        self.append_debug(&dm);
        match self.get_buffer_idx(&buffer_name) {
            Some(idx) => {
                self.buffer_selected_idx = idx;
                let mut curbuf = self
                    .buffer_list
                    .get_mut(self.buffer_selected_idx)
                    .unwrap()
                    .clone();
                curbuf.load_from_disk(false);
                JsonRpc::success(req_id, &self.get_buffer_value(self.buffer_selected_idx))
            }
            None => {
                let mut error = JRError::invalid_params();
                let details = format!("buffer '{}' does not exist", buffer_name);
                error.data = Some(Value::from(details.to_string()));
                JsonRpc::error(req_id, error)
            }
        }
    }

    fn handle_delete_buffer(&mut self, message: &JsonRpc) -> (JsonRpc, Option<JsonRpc>) {
        let req_id = message.get_id().unwrap();
        let buffer_name = match message.get_params().unwrap() {
            Params::Array(args) => args[0].as_str().unwrap().to_owned(),
            _ => String::new(),
        };
        let dm = format!("buffer-delete: {}", buffer_name);
        self.append_debug(&dm);
        match self.get_buffer_idx(&buffer_name) {
            Some(idx) => {
                self.delete_buffer(idx);
                let mut params: Map<String, Value> = Map::new();
                params.insert(String::from("buffer_deleted"), Value::from(buffer_name));
                (
                    JsonRpc::success(req_id, &Value::from(params)),
                    Some(self.notification_buffer_changed()),
                )
            }
            None => {
                let mut error = JRError::invalid_params();
                let details = format!("buffer '{}' does not exist", buffer_name);
                error.data = Some(Value::from(details.to_string()));
                (JsonRpc::error(req_id, error), None)
            }
        }
    }
}
