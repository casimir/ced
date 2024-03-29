use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::{cell::RefCell, env::current_dir};
use std::{
    collections::{HashMap, HashSet},
    sync::MutexGuard,
};

use crate::editor::selection::Selection;
use crate::editor::view::{Focus, Lens, View};
use crate::editor::{Buffer, Coords, EditorInfo};
use crate::server::BroadcastMessage;
use crate::stackmap::StackMap;
use async_channel::Sender;
use futures_lite::*;
use remote::jsonrpc::Notification;
use remote::protocol::{
    notifications::{self, Notification as _},
    Face, Text, TextFragment,
};

pub const BUFFER_DEBUG: &str = "*debug*";
pub const BUFFER_SCRATCH: &str = "*scratch*";

#[derive(Clone)]
pub struct Notifier {
    sender: Sender<BroadcastMessage>,
}

impl Notifier {
    pub fn broadcast(&self, message: Notification, only_clients: impl Into<Option<Vec<usize>>>) {
        let bm = match only_clients.into() {
            Some(cs) => BroadcastMessage::for_clients(cs, message),
            None => BroadcastMessage::new(message),
        };
        future::block_on(self.sender.send(bm)).expect("broadcast message");
    }

    pub fn notify(&self, client_id: usize, message: Notification) {
        self.broadcast(message, vec![client_id]);
    }

    fn echo(&self, client_id: impl Into<Option<usize>>, text: &str, face: Face) {
        let params = Text::from(TextFragment {
            text: text.to_owned(),
            face,
        });
        let notif = notifications::Echo::new(params);
        match client_id.into() {
            Some(id) => self.notify(id, notif),
            None => self.broadcast(notif, None),
        }
    }

    pub fn message(&self, client_id: impl Into<Option<usize>>, text: &str) {
        self.echo(client_id, text, Face::Default);
    }

    pub fn error(&self, client_id: impl Into<Option<usize>>, text: &str) {
        self.echo(client_id, text, Face::Error);
    }

    pub fn info_update(&self, client_id: usize, info: &EditorInfo) {
        let params = notifications::InfoParams {
            client: client_id.to_string(),
            session: info.session.to_owned(),
            cwd: info.cwd.display().to_string(),
        };
        self.notify(client_id, notifications::Info::new(params));
    }

    pub fn view_update(&self, params: Vec<(usize, notifications::ViewParams)>) {
        for (client_id, np) in params {
            self.notify(client_id, notifications::View::new(np));
        }
    }
}

impl From<Sender<BroadcastMessage>> for Notifier {
    fn from(sender: Sender<BroadcastMessage>) -> Self {
        Notifier { sender }
    }
}

#[derive(Debug)]
pub enum CursorTarget {
    Left,
    Right,
    Up,
    Down,
    Begin,
    End,
    LineBegin,
    LineEnd,
}

#[derive(Clone, Debug)]
struct ClientContext {
    view: Rc<RefCell<View>>,
    selections: HashMap<String, HashMap<String, Vec<Selection>>>,
}

struct CoreState {
    cwd: PathBuf,
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

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
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
                cwd: current_dir().unwrap_or_else(|_| dirs::home_dir().unwrap_or_default()),
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

    pub fn cwd(&self) -> PathBuf {
        lock!(self).cwd.to_owned()
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

    fn get_selected_text(&self, buffer: &str, sel: &Selection) -> Option<String> {
        let state = lock!(self);
        state
            .buffers
            .get(buffer)
            .map(|b| b.content.text_range(&b.selection_range(sel)))
            .flatten()
    }

    fn get_coord(&self, buffer: &str, offset: usize) -> Option<Coords> {
        lock!(self)
            .buffers
            .get(buffer)
            .map(|b| b.content.offset_to_coord(offset))
            .flatten()
    }

    fn notify_view_update(&self, clients: Vec<usize>) {
        let state = lock!(self);
        let params = clients
            .iter()
            .map(|id| {
                let view = state.clients[&id].view.borrow();
                let sels = state.clients[&id].selections.get(&view.key());
                (*id, view.to_notification_params(&state.buffers, sels))
            })
            .collect();
        self.notifier.view_update(params);
    }

    fn append_to(&mut self, buffer: &str, text: String) {
        if !self.buffer_exists(buffer) {
            self.open_scratch(buffer, text);
        } else if let Some(buf) = lock!(self).buffers.get_mut(buffer) {
            buf.append(text);
        }
        self.notify_view_update(self.clients_with_buffer(buffer));
    }

    pub fn debug(&mut self, content: &str) {
        log::debug!("{}", content);

        if self.debug_mode {
            self.append_to(BUFFER_DEBUG, format!("{}\n", content));
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
        self.append_to("*messages*", msg + "\n");
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
        self.append_to("*errors*", msg + "\n");
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
            state.clients.insert(id, context);
        }
        self.debug(&format!("new client: {}", id));
        self.notifier.info_update(id, info);
    }

    pub fn remove_client(&mut self, id: usize) {
        lock!(self).clients.remove(&id);
        self.debug(&format!("client left: {}", id));
    }

    pub fn open_scratch(&mut self, name: &str, content: String) {
        let buffer = Buffer::new_scratch(name.to_owned(), content);
        lock!(self).buffers.insert(name.to_owned(), buffer);
    }

    pub fn open_file(&mut self, buffer_name: &str, filename: &Path) {
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
                self.open_scratch(BUFFER_SCRATCH, String::new());
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

    pub fn edit(&mut self, client_id: usize, name: &str, scratch: bool) {
        // TODO check for same file but different name
        let exists = self.buffer_exists(name);
        let notify_change = if scratch {
            if !exists {
                self.open_scratch(name, String::new());
            }
            false
        } else if exists {
            let reloaded = lock!(self).buffers.get_mut(name).unwrap().load_from_disk();
            if reloaded {
                self.debug(&format!("reloaded from disk: {}", name));
            }
            reloaded
        } else {
            let path = {
                let mut absolute = self.cwd();
                absolute.push(name);
                absolute
            };
            self.open_file(name, &path);
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

    pub fn move_cursor(&mut self, client_id: usize, direction: CursorTarget, extend: bool) {
        let ctx = lock!(self).clients[&client_id].clone();
        let curview = ctx.view.borrow().key();
        let mut selections = ctx.selections[&curview].clone();
        let mut moved = false;
        for (b, bss) in selections.iter_mut() {
            let buffer = &lock!(self).buffers[b];
            if buffer.content.len() == 1 {
                // there is absolutly nowhere to move out to, let's bail out
                continue;
            }
            for s in bss.iter_mut() {
                let original = &s.clone();
                let coord = buffer.content.offset_to_coord(s.cursor);
                let mut nv = buffer.content.navigate(coord).unwrap();
                nv.target_col = s.target_col;
                match direction {
                    CursorTarget::Left => nv.previous(),
                    CursorTarget::Right => nv.next(),
                    CursorTarget::Up => nv.previous_line(),
                    CursorTarget::Down => nv.next_line(),
                    CursorTarget::LineBegin => nv.line_begin(),
                    CursorTarget::LineEnd => nv.line_end(),
                    CursorTarget::Begin => nv.begin(),
                    CursorTarget::End => nv.end(),
                };
                s.cursor = nv.pos().offset;
                s.target_col = nv.target_col;
                if !extend {
                    s.anchor = s.cursor
                }
                moved |= s != original;
            }
        }
        lock!(self)
            .clients
            .get_mut(&client_id)
            .unwrap()
            .selections
            .insert(curview, selections);
        if moved {
            self.notify_view_update(vec![client_id]);
        }
    }

    fn clamp_selections(state: &mut MutexGuard<CoreState>, client_id: usize, bufname: &str) {
        let max_offset = state.buffers[bufname].content.max_offset();
        let ctx = state.clients.get_mut(&client_id).unwrap();
        let view_key = ctx.view.borrow().key();
        ctx.selections
            .get_mut(&view_key)
            .unwrap()
            .get_mut(bufname)
            .unwrap()
            .iter_mut()
            .for_each(|s| {
                s.clamp_to(max_offset);
            });
    }

    pub fn delete_selection(&mut self, client_id: usize) -> Vec<String> {
        let mut deleted = Vec::new();
        {
            let mut modified_buffers = HashSet::new();
            let mut state = lock!(self);
            let ctx = &state.clients[&client_id];
            let view_key = ctx.view.borrow().key();
            for (bufname, sels) in &ctx.selections[&view_key].clone() {
                let buffer = state
                    .buffers
                    .get_mut(bufname)
                    .unwrap_or_else(|| panic!("invalid buffer: {}", bufname));
                for sel in sels {
                    let srange = buffer.selection_range(sel);
                    if let Some(text) = buffer.content.text_range(&srange) {
                        buffer.content.delete(&srange);
                        deleted.push(text);
                        modified_buffers.insert(bufname.to_owned());
                    }
                }
                let buffer = &mut state.buffers.get_mut(bufname).unwrap();
                if buffer.content.is_empty() {
                    buffer.content.append("\n".to_owned());
                }
            }
            // XXX selections reprocessing is made afterwards because of the borrow rules on struct attributes
            // XXX in 2021 edition it should be possible to handle both operations in one pass
            for bufname in modified_buffers {
                Self::clamp_selections(&mut state, client_id, &bufname);
            }
        }
        if !deleted.is_empty() {
            self.notify_view_update(vec![client_id]);
        }
        deleted
    }
}

unsafe impl Send for Core {}

struct PositionData {
    offset: usize,
    pos: Coords,
}

impl<'lua> rlua::ToLua<'lua> for PositionData {
    fn to_lua(self, lua: rlua::Context<'lua>) -> rlua::Result<rlua::Value<'lua>> {
        let t_pos = lua.create_table()?;
        t_pos.set("c", self.pos.c)?;
        t_pos.set("l", self.pos.l)?;

        let t = lua.create_table()?;
        t.set("offset", self.offset)?;
        t.set("pos", t_pos)?;
        Ok(rlua::Value::Table(t))
    }
}

struct SelectionData {
    anchor: PositionData,
    cursor: PositionData,
    text: String,
}

impl<'lua> rlua::ToLua<'lua> for SelectionData {
    fn to_lua(self, lua: rlua::Context<'lua>) -> rlua::Result<rlua::Value<'lua>> {
        let t = lua.create_table()?;
        t.set("anchor", self.anchor)?;
        t.set("cursor", self.cursor)?;
        t.set("text", self.text)?;
        Ok(rlua::Value::Table(t))
    }
}

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

        methods.add_method("get_context", |lua, this, client: usize| {
            let context = lock!(this).clients[&client].clone();
            let view_key = context.view.borrow().key();
            let sels = &context.selections[&view_key];

            let view_t = lua.create_table()?;
            view_t.set("key", view_key)?;

            let selections_t = lua.create_table()?;
            for (k, v) in sels {
                let data: Vec<SelectionData> = v
                    .iter()
                    .map(|s| SelectionData {
                        anchor: PositionData {
                            offset: s.anchor,
                            pos: this.get_coord(k, s.anchor).unwrap(),
                        },
                        cursor: PositionData {
                            offset: s.cursor,
                            pos: this.get_coord(k, s.cursor).unwrap(),
                        },
                        text: this.get_selected_text(k, s).unwrap_or_default(), // FIXME Option?
                    })
                    .collect();
                selections_t.set(k.as_str(), data)?;
            }

            let t = lua.create_table()?;
            t.set("view", view_t)?;
            t.set("selections", selections_t)?;
            Ok(t)
        });

        methods.add_method_mut(
            "scratch",
            |_, this, (client, name, content): (usize, String, String)| {
                this.open_scratch(&name, content);
                this.edit(client, &name, true);
                Ok(())
            },
        );
        methods.add_method_mut(
            "edit",
            |_, this, (client, name, scratch): (usize, String, bool)| {
                this.edit(client, &name, scratch);
                Ok(())
            },
        );
        methods.add_method_mut("append_to", |_, this, (buffer, text): (String, String)| {
            this.append_to(&buffer, text);
            Ok(())
        });

        methods.add_method_mut("move_left", |_, this, (client, extend)| {
            this.move_cursor(client, CursorTarget::Left, extend);
            Ok(())
        });
        methods.add_method_mut("move_right", |_, this, (client, extend)| {
            this.move_cursor(client, CursorTarget::Right, extend);
            Ok(())
        });
        methods.add_method_mut("move_up", |_, this, (client, extend)| {
            this.move_cursor(client, CursorTarget::Up, extend);
            Ok(())
        });
        methods.add_method_mut("move_down", |_, this, (client, extend)| {
            this.move_cursor(client, CursorTarget::Down, extend);
            Ok(())
        });
        methods.add_method_mut("move_to_line_begin", |_, this, (client, extend)| {
            this.move_cursor(client, CursorTarget::LineBegin, extend);
            Ok(())
        });
        methods.add_method_mut("move_to_line_end", |_, this, (client, extend)| {
            this.move_cursor(client, CursorTarget::LineEnd, extend);
            Ok(())
        });
        methods.add_method_mut("move_to_begin", |_, this, (client, extend)| {
            this.move_cursor(client, CursorTarget::Begin, extend);
            Ok(())
        });
        methods.add_method_mut("move_to_end", |_, this, (client, extend)| {
            this.move_cursor(client, CursorTarget::End, extend);
            Ok(())
        });
        methods.add_method_mut("delete_selection", |_, this, client| {
            let deleted = this.delete_selection(client);
            Ok(deleted)
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
