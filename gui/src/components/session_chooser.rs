use std::cell::RefCell;

use ced_remote::Session;
use gtk::{
    BoxExt, ButtonExt, ContainerExt, EntryExt, GtkWindowExt, ListBoxExt, ListBoxRowExt, ObjectExt,
    WidgetExt,
};

use crate::components::show_new_client;

thread_local! {
    static CHOOSER: RefCell<Option<gtk::ApplicationWindow>> = RefCell::new(None);
}

pub fn show_session_chooser(application: &gtk::Application) {
    info!("start session chooser");
    let window = gtk::ApplicationWindow::new(application);
    window.set_title("Session chooser");
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(200, 200);
    window.connect_delete_event(|_, _| {
        CHOOSER.with(|global| *global.borrow_mut() = None);
        gtk::Inhibit(false)
    });

    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let session_list = gtk::ListBox::new();
    session_list.set_placeholder(&gtk::Label::new("No active session"));
    let sessions = Session::list();
    for session in &sessions {
        let label = gtk::Label::new(session.as_str());
        session_list.add(&label);
    }
    let sessions_clone = sessions.clone();
    let app_wk = application.downgrade();
    let validate_button = gtk::Button::new_with_label("Connect");
    validate_button.connect_clicked(clone!(session_list => move |_| {
        session_list.get_selected_row().map(|row| {
            let session = &sessions_clone[row.get_index() as usize];
            app_wk.upgrade().map(|app| {
                show_new_client(&app, &session);
                CHOOSER.with(|global| {
                    if let Some(ref win) = *global.borrow() {
                        win.destroy();
                    }
                });
                CHOOSER.with(|global| *global.borrow_mut() = None);
            });
        });
    }));
    container.pack_start(&session_list, true, true, 5);
    container.pack_start(&validate_button, false, false, 5);

    container.pack_start(
        &gtk::Separator::new(gtk::Orientation::Vertical),
        false,
        true,
        5,
    );

    let new_container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let entry = gtk::Entry::new();
    new_container.pack_start(&entry, true, true, 5);
    let app_wk = application.downgrade();
    let button = gtk::Button::new_with_label("+");
    entry.connect_activate(clone!(button => move |_| {
        button.emit_clicked();
    }));
    button.connect_clicked(move |_| {
        app_wk.upgrade().map(|app| {
            let session = entry.get_buffer().get_text();
            show_new_client(&app, &session);
            CHOOSER.with(|global| {
                if let Some(ref win) = *global.borrow() {
                    win.destroy();
                }
            });
            CHOOSER.with(|global| *global.borrow_mut() = None);
        });
    });
    new_container.pack_start(&button, false, true, 5);
    container.pack_start(&new_container, false, true, 5);

    container.show_all();
    window.add(&container);
    window.show_all();
    CHOOSER.with(|global| *global.borrow_mut() = Some(window));
}
