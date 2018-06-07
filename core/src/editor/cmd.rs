use jsonrpc_lite::JsonRpc;
use serde_json::{Map, Value};

use super::Editor;

type CommandHandler = fn(&mut Editor, &JsonRpc) -> (JsonRpc, Option<JsonRpc>);

#[derive(Copy)]
pub struct Command<'a> {
    pub name: &'a str,
    pub help: &'a str,
    pub exec: CommandHandler,
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

pub static COMMAND_MAP: [Command; 5] = [
    Command {
        name: "command-list",
        help: "list available commands",
        exec: command_list,
    },
    Command {
        name: "edit",
        help: "edit a file, reload it from the disk if needed",
        exec: edit,
    },
    Command {
        name: "buffer-list",
        help: "list open buffers (with content)",
        exec: buffer_list,
    },
    Command {
        name: "buffer-select",
        help: "select a buffer by its name",
        exec: buffer_select,
    },
    Command {
        name: "buffer-delete",
        help: "delete a buffer by its name",
        exec: buffer_delete,
    },
];

pub fn command_list(editor: &mut Editor, message: &JsonRpc) -> (JsonRpc, Option<JsonRpc>) {
    let req_id = message.get_id().unwrap();
    let mut result = Map::new();
    for (name, cmd) in &editor.commands {
        result.insert(String::from(*name), Value::from(cmd.help));
    }
    (JsonRpc::success(req_id, &Value::from(result)), None)
}

pub fn edit(editor: &mut Editor, message: &JsonRpc) -> (JsonRpc, Option<JsonRpc>) {
    (editor.handle_edit(message), None)
}

pub fn buffer_list(editor: &mut Editor, message: &JsonRpc) -> (JsonRpc, Option<JsonRpc>) {
    (editor.handle_list_buffer(message), None)
}

pub fn buffer_select(editor: &mut Editor, message: &JsonRpc) -> (JsonRpc, Option<JsonRpc>) {
    (editor.handle_select_buffer(message), None)
}

pub fn buffer_delete(editor: &mut Editor, message: &JsonRpc) -> (JsonRpc, Option<JsonRpc>) {
    editor.handle_delete_buffer(message)
}
