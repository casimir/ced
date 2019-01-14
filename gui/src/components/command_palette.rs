use ced_remote::protocol::{Face, TextFragment};
use ced_remote::Menu;
use gtk::{
    BoxExt, ContainerExt, EditableExt, EntryExt, GtkWindowExt, LabelExt, ListBoxExt,
    SearchEntryExt, WidgetExt,
};

use crate::State;

fn format_palette_entry(fragments: &[TextFragment]) -> String {
    fragments
        .iter()
        .map(|f| match f.face {
            Face::Match => format!("<span weight=\"bold\">{}</span>", f.text,),
            _ => f.text.clone(),
        })
        .collect::<Vec<String>>()
        .join("")
}

pub struct CommandPalette {
    window: gtk::Window,
    entry: gtk::SearchEntry,
    commands: gtk::ListBox,
}

impl CommandPalette {
    pub fn new(window_id: usize, parent: &gtk::ApplicationWindow) -> CommandPalette {
        let window = gtk::Window::new(gtk::WindowType::Popup);
        window.set_default_size(300, 300);
        window.set_type_hint(gdk::WindowTypeHint::Dialog);
        window.set_transient_for(parent);
        window.set_skip_taskbar_hint(true);
        window.set_modal(true);
        window.set_destroy_with_parent(true);
        window.set_position(gtk::WindowPosition::CenterOnParent);

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let commands = gtk::ListBox::new();
        commands.set_can_focus(false);

        let entry = gtk::SearchEntry::new();
        entry.connect_search_changed(move |entry| {
            State::with(window_id, |state, _| {
                let search = &entry.get_text().unwrap_or(String::new());
                state.update_menu_search(search);
            });
        });
        entry.connect_stop_search(clone!(window => move |_| {
            window.destroy();
            State::with(window_id, |state, _| {
                state.cancel_menu();
            });
        }));
        entry.connect_previous_match(clone!(commands => move |_| {
            State::with(window_id, |state, _| {
                    state.previous_menu_entry().map(|index| {
                    commands
                        .get_row_at_index(index as i32)
                        .map(|row| commands.select_row(&row));
                });
            });
        }));
        entry.connect_next_match(clone!(commands => move |_| {
            State::with(window_id, |state, _| {
                    state.next_menu_entry().map(|index| {
                    commands
                        .get_row_at_index(index as i32)
                        .map(|row| commands.select_row(&row));
                });
            });
        }));
        entry.connect_key_press_event(move |entry, event| match event.get_keyval() {
            gdk::enums::key::Up | gdk::enums::key::KP_Up => {
                entry.emit_previous_match();
                gtk::Inhibit(true)
            }
            gdk::enums::key::Down | gdk::enums::key::KP_Down => {
                entry.emit_next_match();
                gtk::Inhibit(true)
            }
            _ => gtk::Inhibit(false),
        });
        entry.connect_activate(clone!(window => move |_| {
            window.destroy();
            State::with(window_id, |state, _| {
                state.select_menu_entry();
            });
        }));
        container.pack_start(&entry, false, true, 5);

        let scrolled_commands = gtk::ScrolledWindow::new(None, None);
        scrolled_commands.add(&commands);
        container.pack_start(&scrolled_commands, true, true, 5);

        window.add(&container);
        container.show_all();

        CommandPalette {
            window,
            entry,
            commands,
        }
    }

    pub fn show(&self) {
        self.window.show_all();
    }

    pub fn update(&self, menu: &Menu) {
        self.entry.get_buffer().set_text(menu.search.as_str());
        self.entry.set_position(-1);

        for child in self.commands.get_children() {
            self.commands.remove(&child);
        }
        for entry in &menu.entries {
            let label = gtk::Label::new(None);
            label.set_halign(gtk::Align::Start);
            let markup = format_palette_entry(&entry.fragments);
            match &entry.description {
                Some(desc) => {
                    let with_desc = format!("{} ({})", markup, desc);
                    label.set_markup(&with_desc);
                }
                None => label.set_markup(&markup),
            }
            self.commands.add(&label);
        }
        self.commands
            .get_row_at_index(menu.selected as i32)
            .map(|row| self.commands.select_row(&row));
    }
}
