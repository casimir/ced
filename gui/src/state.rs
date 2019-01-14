use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::Receiver;

use ced_remote::protocol::notification::view::{Params as View, ParamsItem as ViewItem};
use ced_remote::{Connection, ConnectionEvent, Menu};
use gtk::{ContainerExt, LabelExt, WidgetExt};

use crate::components::CommandPalette;

thread_local! {
    static STATES: RefCell<HashMap<usize, State>> = RefCell::new(HashMap::new());
}

pub struct State {
    pub window: gtk::ApplicationWindow,
    main_view: gtk::Box,
    connection: Connection,
    pub events: Receiver<ConnectionEvent>,
    palette: Option<CommandPalette>,
    last_menu_search: Option<String>,
}

impl State {
    pub fn new(
        window: gtk::ApplicationWindow,
        main_view: gtk::Box,
        connection: Connection,
        events: Receiver<ConnectionEvent>,
    ) -> State {
        State {
            window,
            main_view,
            connection,
            events,
            palette: None,
            last_menu_search: None,
        }
    }

    pub fn ids() -> Vec<usize> {
        STATES.with(|global| global.borrow().keys().cloned().collect())
    }

    pub fn next_id() -> usize {
        STATES.with(|global| {
            global
                .borrow()
                .keys()
                .fold(1, |curr, key| std::cmp::max(curr, *key))
                + 1
        })
    }

    pub fn register(window_id: usize, state: State) {
        STATES.with(move |global| global.borrow_mut().insert(window_id, state));
    }

    pub fn with<F, R>(window_id: usize, f: F) -> R
    where
        F: FnOnce(&mut State, usize) -> R,
    {
        STATES.with(|global| {
            let mut states = global.borrow_mut();
            let count = states.len();
            let state = states.get_mut(&window_id).expect("access client state");
            f(state, count)
        })
    }

    pub fn close_connection(&mut self) {
        self.connection.quit();
    }

    pub fn refresh_view(&self, view: &View) {
        for child in self.main_view.get_children() {
            self.main_view.remove(&child);
        }

        for item in view {
            match item {
                ViewItem::Header(header) => {
                    let label = gtk::Label::new(None);
                    let markup = format!(
                        "<span weight=\"bold\">{} [{}:{}]</span>",
                        header.buffer, header.start, header.end
                    );
                    label.set_markup(&markup);
                    label.set_halign(gtk::Align::Start);
                    self.main_view.add(&label);
                }
                ViewItem::Lines(content) => {
                    for line in &content.lines {
                        let label = gtk::Label::new(line.as_str());
                        label.set_halign(gtk::Align::Start);
                        self.main_view.add(&label);
                    }
                }
            }
        }
        self.main_view.show_all();
    }

    pub fn refresh_command_palette(&mut self, menu: &Menu, window_id: usize) {
        let palette = self
            .palette
            .take()
            .unwrap_or(CommandPalette::new(window_id, &self.window));
        palette.update(&menu);
        palette.show();
        self.palette = Some(palette);
    }

    pub fn start_menu(&mut self, command: &str) {
        self.connection.menu(command, "");
    }

    pub fn update_menu_search(&mut self, search: &String) {
        if self.last_menu_search.as_ref() != Some(search) {
            let menu = self.connection.state().menu.expect("fetch current menu");
            self.connection.menu(&menu.command, search);
            self.last_menu_search = Some(search.to_string())
        }
    }

    pub fn cancel_menu(&mut self) {
        self.last_menu_search = None;
        self.palette = None;
    }

    pub fn select_menu_entry(&mut self) {
        self.connection.menu_select();
        self.cancel_menu();
    }

    pub fn previous_menu_entry(&mut self) -> Option<usize> {
        self.connection.action_menu_select_previous();
        self.connection.state().menu.map(|menu| menu.selected)
    }

    pub fn next_menu_entry(&mut self) -> Option<usize> {
        self.connection.action_menu_select_next();
        self.connection.state().menu.map(|menu| menu.selected)
    }
}
