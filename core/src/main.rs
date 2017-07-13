#[macro_use]
extern crate clap;
extern crate jsonrpc_lite;
extern crate serde;
extern crate serde_json;

use std::fs::File;
use std::io;
use std::io::{BufRead, Read, Write};
use std::ops::IndexMut;
use std::path::PathBuf;

use clap::{Arg, App};
use jsonrpc_lite::{JsonRPC, Id, Params, Error, ErrorCode};
use serde_json::value::{Map, Value};
use serde_json::value::ToJson;

struct Buffer {
    name: String,
    file_path: Option<PathBuf>,
    content: String,
}

impl ToString for Buffer {
    fn to_string(&self) -> String {
        self.content.clone()
    }
}

impl Buffer {
    fn append(&mut self, content: &str) {
        self.content.push_str(content);
    }
}

impl<'a> From<&'a Buffer> for Value {
    fn from(buffer: &Buffer) -> Self {
        let mut val: Map<String, Value> = Map::new();
        val.insert(String::from("name"), Value::from(buffer.name.clone()));
        val.insert(String::from("content"), Value::from(buffer.to_string()));
        Value::from(val)
    }
}

struct Editor {
    buffer_list: Vec<Buffer>,
    buffer_selected_idx: usize,
}

impl Editor {
    fn new(filenames: Vec<&str>) -> Editor {
        let mut editor = Editor {
            buffer_list: Vec::new(),
            buffer_selected_idx: 0,
        };
        editor.new_scratch("*debug*");
        if filenames.len() == 0 {
            editor.new_scratch("*scratch*");
        } else {
            for filename in &filenames {
                editor.open_file(PathBuf::from(filename));
            }
        }
        editor
    }

    fn new_scratch(&mut self, name: &str) {
        let buffer = Buffer {
            name: name.to_owned(),
            file_path: None,
            content: String::new(),
        };
        self.buffer_list.push(buffer);
        self.buffer_selected_idx = self.buffer_list.len() - 1;
    }

    fn open_file(&mut self, filename: PathBuf) {
        let mut full_path = std::env::current_dir().unwrap();
        full_path.push(filename.clone());
        let absolute_path = full_path.as_path().canonicalize().unwrap();

        let message = format!("open file: {:?}\n", absolute_path);
        self.append_debug(&message);

        let mut file = File::open(&absolute_path).unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content);

        let buffer = Buffer {
            name: filename.into_os_string().into_string().unwrap(),
            file_path: Some(absolute_path),
            content: content,
        };
        self.buffer_list.push(buffer);
        self.buffer_selected_idx = self.buffer_list.len() - 1;
    }

    fn delete_buffer(&mut self, idx: usize) {
        if idx != 0 && idx >= self.buffer_selected_idx {
            self.buffer_selected_idx -= 1;
        }
        self.buffer_list.remove(idx);
        if self.buffer_list.len() == 0 {
            self.new_scratch("*scratch*");
        }
    }

    fn get_buffer_idx(&self, buffer_name: &str) -> Option<usize> {
        self.buffer_list.iter().position(|x| x.name == buffer_name)
    }

    fn get_buffer_idx_from_path(&self, path: &PathBuf) -> Option<usize> {
        self.buffer_list.iter().position(
            |x| x.file_path == Some(path.clone()),
        )
    }

    fn append_debug(&mut self, content: &str) {
        let mut debug_buffer = self.buffer_list.index_mut(0);
        debug_buffer.append(content);
        writeln!(io::stderr(), "{}", content);
    }

    fn send_message(&self, message: &JsonRPC) {
        let json = message.to_json().unwrap();
        let payload = serde_json::to_string(&json).unwrap();
        println!("{}", payload);
    }

    fn send_update(&self) {
        let mut params: Map<String, Value> = Map::new();
        params.insert(
            String::from("buffer_list"),
            Value::from(
                self.buffer_list
                    .iter()
                    .map(|x| Value::from(x))
                    .collect::<Vec<Value>>(),
            ),
        );
        let buffer_current = self.buffer_list.get(self.buffer_selected_idx).unwrap();
        let buffer_current_name = buffer_current.name.clone();
        params.insert(
            String::from("buffer_current"),
            Value::from(buffer_current_name),
        );
        let message = JsonRPC::notification_with_params("update", &Params::Map(params));
        self.send_message(&message);
    }

    fn handle(&mut self, line: &str) {
        for message in JsonRPC::parse(line) {
            let to_write = match message.get_method() {
                Some("edit") => self.handle_edit(&message),
                Some("buffer_delete") => self.handle_delete_buffer(&message),
                Some("buffer_list") => self.handle_list_buffer(&message),
                Some("buffer_select") => self.handle_select_buffer(&message),
                _ => {
                    let dm = format!("unknown command: {:?}\n", message);
                    self.append_debug(&dm);
                    let req_id = message.get_id().unwrap();
                    JsonRPC::error(&req_id, &Error::method_not_found())
                }
            };
            self.send_message(&to_write);
        }
    }

    fn handle_edit(&mut self, message: &JsonRPC) -> JsonRPC {
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
        let ref buffer_ref = self.buffer_list[self.buffer_selected_idx];
        JsonRPC::success(&req_id, &Value::from(buffer_ref))
    }

    fn handle_list_buffer(&self, message: &JsonRPC) -> JsonRPC {
        let req_id = message.get_id().unwrap();
        let params = self.buffer_list
            .iter()
            .map(|x| (Value::from(x)))
            .collect::<Vec<Value>>();
        JsonRPC::success(&req_id, &Value::from(params))
    }

    fn handle_select_buffer(&mut self, message: &JsonRPC) -> JsonRPC {
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
                let ref buffer_ref = self.buffer_list[idx];
                JsonRPC::success(&req_id, &Value::from(buffer_ref))
            }
            None => {
                let mut error = Error::invalid_params();
                let details = format!("buffer '{}' does not exist", buffer_name);
                error.data = Some(Value::from(details.to_string()));
                JsonRPC::error(&req_id, &error)
            }
        }
    }

    fn handle_delete_buffer(&mut self, message: &JsonRPC) -> JsonRPC {
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
                let ref buffer_ref = self.buffer_list[self.buffer_selected_idx];
                JsonRPC::success(&req_id, &Value::from(buffer_ref))
            }
            None => {
                let mut error = Error::invalid_params();
                let details = format!("buffer '{}' does not exist", buffer_name);
                error.data = Some(Value::from(details.to_string()));
                JsonRPC::error(&req_id, &error)
            }
        }
    }
}

fn main() {
    let matches = App::new("ced")
        .about(crate_description!())
        .version(crate_version!())
        .author(crate_authors!())
        .arg(Arg::with_name("FILE").multiple(true).help(
            "a list of file to open",
        ))
        .get_matches();

    let files = match matches.values_of("FILE") {
        Some(args) => args.collect(),
        None => Vec::new(),
    };
    let mut editor = Editor::new(files);

    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut buf = String::new();
    editor.send_update();
    while handle.read_line(&mut buf).is_ok() {
        if buf.is_empty() {
            break;
        }
        editor.handle(&buf);
        buf.clear();
    }
}
