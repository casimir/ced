use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crossbeam_channel as channel;
use failure::Error;

use super::range::Range;
use super::view::{Focus, Lens, View};
use super::{Buffer, EditorInfo};
use crate::datastruct::StackMap;
use crate::server::BroadcastMessage;
use remote::jsonrpc::Notification;
use remote::protocol;

#[derive(Clone, Debug)]
struct ClientContext {
    view: Rc<RefCell<View>>,
    selections: HashMap<String, HashMap<String, Range>>,
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

#[derive(Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "buffer not found: {}", name)]
    BufferNotFound { name: String },
    #[fail(display = "view not found: {}", view_id)]
    ViewNotFound { view_id: String },
}

#[derive(Clone)]
pub struct Core {
    broadcaster: channel::Sender<BroadcastMessage>,
    state: Arc<Mutex<CoreState>>,
}

impl Core {
    pub fn new(broadcaster: channel::Sender<BroadcastMessage>) -> Core {
        Core {
            broadcaster,
            state: Arc::new(Mutex::new(CoreState {
                clients: StackMap::new(),
                buffers: HashMap::new(),
                views: StackMap::new(),
            })),
        }
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

    fn broadcast<F>(&self, state: &CoreState, message: Notification, filter: F)
    where
        F: Fn(&usize) -> bool,
    {
        let skiplist = state
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

    fn notify(&self, state: &CoreState, client_id: usize, message: Notification) {
        self.broadcast(state, message, |&k| k == client_id);
    }

    pub fn notify_client(&self, client_id: usize, message: Notification) {
        let state = lock!(self);
        self.notify(&state, client_id, message);
    }

    fn notify_view_update(&self, state: &CoreState, client_id: usize) {
        let context = &state.clients[&client_id];
        self.notify(
            state,
            client_id,
            protocol::notification::view::new(
                context.view.borrow().to_notification_params(&state.buffers),
            ),
        );
    }

    pub fn append_debug(&mut self, content: &str) {
        // TODO better behavior
        // if flag is true
        //    create buffer if absent
        //    append to buffer
        let mut state = lock!(self);
        if let Some(debug_buffer) = state.buffers.get_mut("*debug*") {
            debug_buffer.append(&format!("{}\n", content));
        }
        log::info!("{}", content);
        for (client_id, context) in state.clients.iter() {
            if context.view.borrow().contains_buffer("*debug*") {
                self.notify_view_update(&state, *client_id);
            }
        }
    }

    pub fn add_client(&mut self, id: usize, info: &EditorInfo) {
        // TODO get rid of EditorInfo?
        let context = {
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
                        .map(|&b| (b.clone(), Range::new(0, 1)))
                        .collect(),
                );
                ClientContext {
                    view: Rc::clone(latest_view),
                    selections,
                }
            };
            state.clients.insert(id, context.clone());
            context
        };
        self.append_debug(&format!("new client: {}", id));

        let state = lock!(self);
        self.notify(
            &state,
            id,
            protocol::notification::info::new(id, &info.session, &info.cwd.display().to_string()),
        );
        if !context.view.borrow().contains_buffer("*debug*") {
            self.notify_view_update(&state, id);
        }
    }

    pub fn remove_client(&mut self, id: usize) {
        lock!(self).clients.remove(&id);
        self.append_debug(&format!("client left: {}", id));
    }

    pub fn open_scratch(&mut self, name: &str) {
        let buffer = Buffer::new_scratch(name.to_owned());
        lock!(self).buffers.insert(name.into(), buffer);
    }

    pub fn open_file(&mut self, buffer_name: &str, filename: &PathBuf) {
        let buffer = Buffer::new_file(filename);
        lock!(self).buffers.insert(buffer_name.to_string(), buffer);
    }

    pub fn add_view(&mut self, view: View) {
        lock!(self)
            .views
            .insert(view.key(), Rc::new(RefCell::new(view)));
    }

    pub fn delete_view(&mut self, view_id: &str) -> Result<(), Error> {
        if !self.view_exists(view_id) {
            return Err(ErrorKind::ViewNotFound {
                view_id: view_id.to_owned(),
            }
            .into());
        }

        let view = lock!(self).views.remove(&view_id.to_owned()).unwrap();
        self.append_debug(&format!("delete view: {}", view_id));
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
                self.append_debug(&format!("delete buffer: {}", buffer));
            }
        }
        if lock!(self).views.is_empty() {
            if lock!(self).buffers.is_empty() {
                self.open_scratch("*scratch*");
            }
            let view = View::for_buffer("*scratch*");
            lock!(self)
                .views
                .insert(view.key(), Rc::new(RefCell::new(view)));
        }
        let mut to_notify = Vec::new();
        let latest_view = Rc::clone(lock!(self).views.latest_value().expect("get latest view"));
        for (id, context) in lock!(self).clients.iter_mut() {
            if context.view.borrow().key() == *view_id {
                context.view = Rc::clone(&latest_view);
                to_notify.push(*id);
            }
        }
        let state = lock!(self);
        for id in to_notify {
            self.notify_view_update(&state, id);
        }
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
            if new_view.is_empty() {
                self.delete_view(&old_key).unwrap();
            } else {
                let mut state = self.state.lock().expect("acquire state mutex");
                let view = Rc::new(RefCell::new(new_view));
                state.views.insert(new_key, Rc::clone(&view));
                let mut to_notify = Vec::new();
                for (id, context) in state.clients.iter_mut() {
                    if context.view.borrow().key() == old_key {
                        context.view = Rc::clone(&view);
                        to_notify.push(*id);
                    }
                }
                for id in to_notify {
                    self.notify_view_update(&state, id);
                }
            }
            lock!(self).views.remove(&old_key);
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
                self.append_debug(&format!("reloaded from disk: {}", name));
            }
            reloaded
        } else {
            self.open_file(name, path.expect("target file path"));
            true
        };

        {
            let mut state = lock!(self);
            let view = Rc::new(RefCell::new(View::for_buffer(name)));
            state.views.insert(view.borrow().key(), Rc::clone(&view));
            let context = state.clients.get_mut(&client_id).unwrap();
            context.view = view;
        }

        self.append_debug(&format!("edit: {}", name));
        let state = lock!(self);
        if notify_change {
            for (id, ctx) in state.clients.iter() {
                if ctx.view.borrow().contains_buffer(name) {
                    self.notify_view_update(&state, *id);
                }
            }
        } else {
            self.notify_view_update(&state, client_id);
        }
    }

    pub fn view(&mut self, client_id: usize, view_id: &str) -> Result<(), Error> {
        if self.view_exists(view_id) {
            let mut state = lock!(self);
            let view = Rc::clone(&state.views[view_id]);
            state.clients.get_mut(&client_id).unwrap().view = view;
            self.notify_view_update(&state, client_id);
            Ok(())
        } else if self.buffer_exists(view_id) {
            let mut state = lock!(self);
            let view = Rc::new(RefCell::new(View::for_buffer(view_id)));
            let key = view.borrow().key();
            let context = state.clients.get_mut(&client_id).unwrap();
            context.view = Rc::clone(&view);
            state.views.entry(key).or_insert(view);
            self.notify_view_update(&state, client_id);
            Ok(())
        } else {
            Err(ErrorKind::ViewNotFound {
                view_id: view_id.to_owned(),
            }
            .into())
        }
    }

    pub fn delete_current_view(&mut self, client_id: usize) {
        let view_id = lock!(self).clients[&client_id].view.borrow().key();
        self.delete_view(&view_id).unwrap();
    }

    pub fn add_to_current_view(&mut self, client_id: usize, buffer: &str) -> Result<(), Error> {
        if !self.buffer_exists(buffer) {
            return Err(ErrorKind::BufferNotFound {
                name: buffer.to_owned(),
            }
            .into());
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
            return Err(ErrorKind::BufferNotFound {
                name: buffer.to_owned(),
            }
            .into());
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
        methods.add_method_mut("append_debug", |_, this, content: String| {
            this.append_debug(&content);
            Ok(())
        })
    }
}
