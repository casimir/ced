#[macro_use]
extern crate clap;
extern crate jsonrpc_lite;
extern crate serde_json;

mod buffer;

use std::collections::BTreeMap;
use std::io::{self, BufRead};
use std::ops::{IndexMut, Index};
use std::path::PathBuf;

use clap::{Arg, App};
use jsonrpc_lite::{JsonRpc, Params, Error};
use serde_json::value::{Map, Value};

use buffer::{find_shortest_name, Buffer, BufferSource};

type CommandHandler = fn(&mut Editor, &JsonRpc) -> JsonRpc;

#[derive(Copy)]
struct Command<'a> {
    name: &'a str,
    help: &'a str,
    exec: CommandHandler,
}

impl<'a> Clone for Command<'a> {
    fn clone(&self) -> Self {
        Command {
            name: self.name.clone(),
            help: self.help.clone(),
            exec: self.exec,
        }
    }
}

fn cmd_command_list(editor: &mut Editor, message: &JsonRpc) -> JsonRpc {
    let req_id = message.get_id().unwrap();
    let mut result = Map::new();
    for (name, cmd) in &editor.commands {
        result.insert(String::from(*name), Value::from(cmd.help));
    }
    JsonRpc::success(req_id, &Value::from(result))
}

fn cmd_edit(editor: &mut Editor, message: &JsonRpc) -> JsonRpc {
    editor.handle_edit(message)
}

fn cmd_buffer_list(editor: &mut Editor, message: &JsonRpc) -> JsonRpc {
    editor.handle_list_buffer(message)
}

fn cmd_buffer_select(editor: &mut Editor, message: &JsonRpc) -> JsonRpc {
    editor.handle_select_buffer(message)
}

fn cmd_buffer_delete(editor: &mut Editor, message: &JsonRpc) -> JsonRpc {
    editor.handle_delete_buffer(message)
}

static COMMAND_MAP: [Command; 5] = [
    Command {
        name: "command-list",
        help: "list available commands",
        exec: cmd_command_list,
    },
    Command {
        name: "edit",
        help: "edit a file, reload it from the disk if needed",
        exec: cmd_edit,
    },
    Command {
        name: "buffer-list",
        help: "list open buffers (with content)",
        exec: cmd_buffer_list,
    },
    Command {
        name: "buffer-select",
        help: "select a buffer by its name",
        exec: cmd_buffer_select,
    },
    Command {
        name: "buffer-delete",
        help: "delete a buffer by its name",
        exec: cmd_buffer_delete,
    },
];

struct Editor<'a> {
    commands: BTreeMap<&'a str, Command<'a>>,
    buffer_list: Vec<Buffer>,
    buffer_selected_idx: usize,
}

impl<'a> Editor<'a> {
    fn new(filenames: Vec<&str>) -> Editor {
        let mut editor = Editor {
            commands: BTreeMap::new(),
            buffer_list: Vec::new(),
            buffer_selected_idx: 0,
        };
        for cmd in &COMMAND_MAP {
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
        let mut debug_buffer = self.buffer_list.index_mut(0);
        debug_buffer.append(content);
        eprintln!("{}", content);
    }

    fn send_message(&self, message: &JsonRpc) {
        let json = serde_json::to_value(message).unwrap();
        let payload = serde_json::to_string(&json).unwrap();
        println!("{}", payload);
    }

    fn send_init(&self) {
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
        let message = JsonRpc::notification_with_params("init", params);
        self.send_message(&message);
    }

    fn send_buffer_changed(&self) {
        let params = self.get_buffer_value(self.buffer_selected_idx);
        let message = JsonRpc::notification_with_params("buffer_changed", params);
        self.send_message(&message)
    }

    fn handle(&mut self, line: &str) {
        for message in JsonRpc::parse(line) {
            let to_write = match message.get_method() {
                Some(name) => {
                    match self.commands.clone().get(name) {
                        Some(cmd) => (cmd.exec)(self, &message),
                        None => {
                            let req_id = message.get_id().unwrap();
                            JsonRpc::error(req_id, Error::method_not_found())
                        }
                    }
                }
                _ => {
                    let dm = format!("unknown command: {:?}\n", message);
                    self.append_debug(&dm);
                    let req_id = message.get_id().unwrap();
                    JsonRpc::error(req_id, Error::method_not_found())
                }
            };
            self.send_message(&to_write);
        }
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
            let mut curbuf = &mut self.buffer_list[self.buffer_selected_idx];
            curbuf.load_from_disk(false);
        }
        JsonRpc::success(req_id, &self.get_buffer_value(self.buffer_selected_idx))
    }

    fn handle_list_buffer(&self, message: &JsonRpc) -> JsonRpc {
        let req_id = message.get_id().unwrap();
        let params = self.buffer_list
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
                let mut curbuf = self.buffer_list
                    .get_mut(self.buffer_selected_idx)
                    .unwrap()
                    .clone();
                curbuf.load_from_disk(false);
                JsonRpc::success(req_id, &self.get_buffer_value(self.buffer_selected_idx))
            }
            None => {
                let mut error = Error::invalid_params();
                let details = format!("buffer '{}' does not exist", buffer_name);
                error.data = Some(Value::from(details.to_string()));
                JsonRpc::error(req_id, error)
            }
        }
    }

    fn handle_delete_buffer(&mut self, message: &JsonRpc) -> JsonRpc {
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
                self.send_buffer_changed();
                let mut params: Map<String, Value> = Map::new();
                params.insert(String::from("buffer_deleted"), Value::from(buffer_name));
                JsonRpc::success(req_id, &Value::from(params))
            }
            None => {
                let mut error = Error::invalid_params();
                let details = format!("buffer '{}' does not exist", buffer_name);
                error.data = Some(Value::from(details.to_string()));
                JsonRpc::error(req_id, error)
            }
        }
    }
}

fn main() {
    let matches = App::new("ced")
        .about(crate_description!())
        .version(crate_version!())
        .author(crate_authors!())
        .arg(
            Arg::with_name("FILE")
                .multiple(true)
                .help("a list of file to open"),
        )
        .get_matches();

    let files = match matches.values_of("FILE") {
        Some(args) => args.collect(),
        None => Vec::new(),
    };
    let mut editor = Editor::new(files);

    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut buf = String::new();
    editor.send_init();
    while handle.read_line(&mut buf).is_ok() {
        if buf.is_empty() {
            break;
        }
        editor.handle(&buf);
        buf.clear();
    }
}
