use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::editor::selection::Selection;
use crate::editor::view::{Focus, Lens, View};
use crate::editor::{Buffer, EditorInfo, Notifier};
use crate::stackmap::StackMap;

pub const BUFFER_DEBUG: &str = "*debug*";
pub const BUFFER_SCRATCH: &str = "*scratch*";

#[derive(Clone, Debug)]
struct ClientContext {
    view: Rc<RefCell<View>>,
    selections: HashMap<String, HashMap<String, Vec<Selection>>>,
}

struct CoreState {
    clients: StackMap<usize, ClientContext>,
    buffers: HashMap<String, Buffer>,
    views: StackMap<String, Rc<RefCell<View>>>,
}

macro_rules! lock {
    ($s:ident) => {
        $s.state.lock().expect("lock state mutex")
    };
}

#[derive(Debug)]
pub enum Error {
    BufferNotFound { name: String },
    ViewNotFound { view_id: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match self {
            BufferNotFound { name } => write!(f, "buffer not found: {}", name),
            ViewNotFound { view_id } => write!(f, "view not found: {}", view_id),
        }
    }
}

impl std::error::Error for Error {}

fn normalize_error_message(message: &str) -> String {
    let mut skipping = None;
    message
        .replace("\n", "\\n")
        .replace("\t", " ")
        .chars()
        .fold(String::new(), |acc, x| {
            if Some(x) != skipping {
                skipping = if x.is_whitespace() { Some(x) } else { None };
                acc + &x.to_string()
            } else {
                acc
            }
        })
}

#[derive(Clone)]
pub struct Core {
    state: Arc<Mutex<CoreState>>,
    notifier: Notifier,
    pub debug_mode: bool,
}

impl Core {
    pub fn new(notifier: Notifier) -> Core {
        Core {
            state: Arc::new(Mutex::new(CoreState {
                clients: StackMap::new(),
                buffers: HashMap::new(),
                views: StackMap::new(),
            })),
            notifier,
            debug_mode: true,
        }
    }

    pub fn get_notifier(&self) -> &Notifier {
        &self.notifier
    }

    fn buffer_exists(&self, name: &str) -> bool {
        lock!(self).buffers.contains_key(name)
    }

    pub fn buffers(&self) -> Vec<String> {
        lock!(self).buffers.keys().map(String::to_owned).collect()
    }

    fn view_exists(&self, view_id: &str) -> bool {
        lock!(self).views.contains_key(view_id)
    }

    pub fn views(&self) -> Vec<String> {
        lock!(self).views.keys().map(String::to_owned).collect()
    }

    fn clients_with_buffer(&self, name: &str) -> Vec<usize> {
        if !self.buffer_exists(name) {
            return Vec::new();
        }

        lock!(self)
            .clients
            .iter()
            .filter_map(|(&id, ctx)| {
                if ctx.view.borrow().contains_buffer(name) {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    fn notify_view_update(&self, clients: Vec<usize>) {
        let state = lock!(self);
        let params = clients
            .iter()
            .map(|id| {
                let view = state.clients[&id].view.borrow();
                let selections = state.clients[&id].selections.get(&view.key());
                (*id, view.to_notification_params(&state.buffers, selections))
            })
            .collect();
        self.notifier.view_update(params);
    }

    fn append_to(&mut self, buffer: &str, content: &str) {
        if !self.buffer_exists(buffer) {
            self.open_scratch(buffer);
        }
        if let Some(buf) = lock!(self).buffers.get_mut(buffer) {
            buf.append(&format!("{}\n", content));
        }
        self.notify_view_update(self.clients_with_buffer(buffer));
    }

    pub fn debug(&mut self, content: &str) {
        log::debug!("{}", content);

        if self.debug_mode {
            self.append_to(BUFFER_DEBUG, content);
        }
    }

    pub fn message<C>(&mut self, client: C, content: &str)
    where
        C: Into<Option<usize>>,
    {
        let client_id = client.into();
        let msg = if let Some(id) = client_id {
            format!("{}|{}", id, content)
        } else {
            format!("*|{}", content)
        };
        self.append_to("*messages*", &msg);
        self.notifier.message(client_id, content);
    }

    pub fn error<C>(&mut self, client: C, tag: &str, content: &str)
    where
        C: Into<Option<usize>>,
    {
        log::error!("{}: {}", tag, content);
        let client_id = client.into();
        let text = format!("{}: {}", tag, content);
        let msg = if let Some(id) = client_id {
            format!("{}|{}", id, text)
        } else {
            format!("*|{}", text)
        };
        self.append_to("*errors*", &msg);
        self.notifier
            .error(client_id, &normalize_error_message(&text));
    }

    pub fn add_client(&mut self, id: usize, info: &EditorInfo) {
        {
            let mut state = lock!(self);
            let context = if let Some(c) = state.clients.latest() {
                state.clients[c].clone()
            } else {
                let latest_view = state.views.latest_value().unwrap();
                let mut selections = HashMap::new();
                selections.insert(
                    latest_view.borrow().key(),
                    latest_view
                        .borrow()
                        .buffers()
                        .iter()
                        .map(|&b| (b.clone(), vec![Selection::new()]))
                        .collect(),
                );
                ClientContext {
                    view: Rc::clone(latest_view),
                    selections,
                }
            };
            state.clients.insert(id, context.clone());
        }
        self.debug(&format!("new client: {}", id));
        self.notifier.info_update(id, info);
    }

    pub fn remove_client(&mut self, id: usize) {
        lock!(self).clients.remove(&id);
        self.debug(&format!("client left: {}", id));
    }

    pub fn open_scratch(&mut self, name: &str) {
        let buffer = Buffer::new_scratch(name.to_owned());
        lock!(self).buffers.insert(name.to_owned(), buffer);
    }

    pub fn open_file(&mut self, buffer_name: &str, filename: &PathBuf) {
        let buffer = Buffer::new_file(filename);
        lock!(self).buffers.insert(buffer_name.to_owned(), buffer);
    }

    pub fn add_view(&mut self, view: View) {
        lock!(self)
            .views
            .insert(view.key(), Rc::new(RefCell::new(view.clone())));
        for (_id, context) in lock!(self).clients.iter_mut() {
            let sels_by_view = context
                .selections
                .entry(view.key())
                .or_insert_with(HashMap::new);
            for buffer in view.buffers() {
                sels_by_view
                    .entry(buffer.to_owned())
                    .or_insert_with(|| vec![Selection::new()]);
            }
        }
    }

    pub fn delete_view(&mut self, view_id: &str) -> Result<(), Error> {
        if !self.view_exists(view_id) {
            return Err(Error::ViewNotFound {
                view_id: view_id.to_owned(),
            });
        }

        let view = lock!(self).views.remove(&view_id.to_owned()).unwrap();
        self.debug(&format!("delete view: {}", view_id));
        for (_id, context) in lock!(self).clients.iter_mut() {
            context.selections.remove(view_id);
        }
        for buffer in view.borrow().buffers() {
            let mut has_ref = false;
            for view in lock!(self).views.values() {
                if view.borrow().buffers().iter().any(|&b| b == buffer) {
                    has_ref = true;
                    break;
                }
            }
            if !has_ref {
                lock!(self).buffers.remove(&buffer.to_owned());
                self.debug(&format!("delete buffer: {}", buffer));
            }
        }
        if lock!(self).views.is_empty() {
            if lock!(self).buffers.is_empty() {
                self.open_scratch(BUFFER_SCRATCH);
            }
            self.add_view(View::for_buffer(BUFFER_SCRATCH));
        }
        let mut to_notify = Vec::new();
        let latest_view = Rc::clone(lock!(self).views.latest_value().expect("get latest view"));
        for (id, context) in lock!(self).clients.iter_mut() {
            if context.view.borrow().key() == *view_id {
                context.view = Rc::clone(&latest_view);
                to_notify.push(*id);
            }
        }
        self.notify_view_update(to_notify);
        Ok(())
    }

    pub fn modify_view<F>(&mut self, view_id: &str, f: F)
    where
        F: Fn(&mut View),
    {
        let mut new_view = lock!(self).views[view_id].borrow().clone();
        let old_key = new_view.key();
        f(&mut new_view);
        let new_key = new_view.key();
        if old_key != new_key {
            if !new_view.is_empty() {
                for (_id, context) in lock!(self).clients.iter_mut() {
                    if let Some(old_sels) = context.selections.remove(&old_key) {
                        context.selections.insert(new_key.clone(), old_sels);
                    }
                }
                self.add_view(new_view);
            }
            self.delete_view(&old_key).expect("delete old view");
        }
    }

    pub fn edit(&mut self, client_id: usize, name: &str, path: Option<&PathBuf>, scratch: bool) {
        let exists = self.buffer_exists(name);
        let notify_change = if scratch {
            if !exists {
                self.open_scratch(name);
            }
            false
        } else if exists {
            let reloaded = lock!(self).buffers.get_mut(name).unwrap().load_from_disk();
            if reloaded {
                self.debug(&format!("reloaded from disk: {}", name));
            }
            reloaded
        } else {
            self.open_file(name, path.expect("target file path"));
            true
        };

        let view = View::for_buffer(name);
        let view_id = view.key();
        self.add_view(view);
        self.view(client_id, &view_id).expect("set view after edit");

        self.debug(&format!("edit: {}", name));
        if notify_change {
            let client_ids = self
                .clients_with_buffer(name)
                .into_iter()
                .filter(|&id| id != client_id)
                .collect();
            self.notify_view_update(client_ids);
        }
    }

    pub fn view(&mut self, client_id: usize, view_id: &str) -> Result<(), Error> {
        if self.view_exists(view_id) {
            {
                let mut state = lock!(self);
                let view = Rc::clone(&state.views[view_id]);
                state.clients.get_mut(&client_id).unwrap().view = view;
            }
            self.notify_view_update(vec![client_id]);
            Ok(())
        } else if self.buffer_exists(view_id) {
            {
                let mut state = lock!(self);
                let view = Rc::new(RefCell::new(View::for_buffer(view_id)));
                let key = view.borrow().key();
                let context = state.clients.get_mut(&client_id).unwrap();
                context.view = Rc::clone(&view);
                state.views.entry(key).or_insert(view);
            }
            self.notify_view_update(vec![client_id]);
            Ok(())
        } else {
            Err(Error::ViewNotFound {
                view_id: view_id.to_owned(),
            })
        }
    }

    pub fn delete_current_view(&mut self, client_id: usize) {
        let view_id = lock!(self).clients[&client_id].view.borrow().key();
        self.delete_view(&view_id).unwrap();
    }

    pub fn add_to_current_view(&mut self, client_id: usize, buffer: &str) -> Result<(), Error> {
        if !self.buffer_exists(buffer) {
            return Err(Error::BufferNotFound {
                name: buffer.to_owned(),
            });
        }

        let view_id = lock!(self).clients[&client_id].view.borrow().key();
        self.modify_view(&view_id, |view| {
            view.add_lens(Lens {
                buffer: buffer.to_owned(),
                focus: Focus::Whole,
            });
        });
        Ok(())
    }

    pub fn remove_from_current_view(
        &mut self,
        client_id: usize,
        buffer: &str,
    ) -> Result<(), Error> {
        if !self.buffer_exists(buffer) {
            return Err(Error::BufferNotFound {
                name: buffer.to_owned(),
            });
        }

        let view_id = lock!(self).clients[&client_id].view.borrow().key();
        self.modify_view(&view_id, |view| {
            if view.contains_buffer(buffer) {
                view.remove_lens_group(buffer);
            }
        });
        Ok(())
    }
}

unsafe impl Send for Core {}

impl rlua::UserData for Core {
    fn add_methods<'lua, M: rlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut("debug", |_, this, content: String| {
            this.debug(&content);
            Ok(())
        });
        methods.add_method_mut("message", |_, this, (client, content): (usize, String)| {
            this.message(client, &content);
            Ok(())
        });
        methods.add_method_mut("error", |_, this, (client, content): (usize, String)| {
            this.error(client, "lua", &content);
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_message_normalization() {
        assert_eq!(
            normalize_error_message("just a simple message"),
            "just a simple message"
        );
        assert_eq!(
            normalize_error_message("a complicated message\n\twith lines\n\t\tand lines"),
            "a complicated message\\n with lines\\n and lines"
        );
    }
}
