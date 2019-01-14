use std::sync::mpsc;

use ced_remote::{ensure_session, Connection, Session};
use gio::{ActionMapExt, MenuExt, SimpleActionExt};
use gtk::{ContainerExt, GtkApplicationExt, GtkWindowExt, ObjectExt, WidgetExt};

use crate::components::show_session_chooser;
use crate::State;

pub fn show_new_client(application: &gtk::Application, session_name: &str) {
    let session = Session::from_name(session_name);
    ensure_session("ced", &session).expect("ensure session");
    info!("start new client for session '{}'", session);
    let connection = Connection::new(&session).expect("establish connection");
    let conn_events = connection.connect();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for ev in conn_events {
            tx.send(ev).expect("forward event");
        }
    });

    let window_id = State::next_id();
    let window = gtk::ApplicationWindow::new(application);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(600, 600);

    let menubar = gio::Menu::new();

    let view_menu = gio::Menu::new();
    view_menu.append("Command palette", "app.palette");
    view_menu.append("Open file", "app.open");
    menubar.append_submenu("_View", &view_menu);

    let palette_action = gio::SimpleAction::new("palette", None);
    palette_action.connect_activate(move |_, _| {
        State::with(window_id, |state, _| state.start_menu(""));
    });
    application.add_action(&palette_action);
    let open_action = gio::SimpleAction::new("open", None);
    open_action.connect_activate(move |_, _| {
        State::with(window_id, |state, _| state.start_menu("open"));
    });
    application.add_action(&open_action);

    application.set_menubar(&menubar);
    application.set_accels_for_action("app.palette", &["<Primary>P"]);
    application.set_accels_for_action("app.open", &["<Primary>O"]);

    let main_view = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scrolled_main_view = gtk::ScrolledWindow::new(None, None);
    scrolled_main_view.add(&main_view);
    window.add(&scrolled_main_view);

    let wk_app = application.downgrade();
    window.connect_delete_event(move |win, _| {
        State::with(window_id, |state, count| {
            info!("delete window and client '{}'", window_id);
            state.close_connection();
            if count <= 1 {
                println!("cnt {}", count);
                // FIXME the closed session still appears here
                // FIXME not triggered on second window?!
                wk_app.upgrade().map(|app| {
                    show_session_chooser(&app);
                });
            }
        });
        win.destroy();
        gtk::Inhibit(false)
    });

    window.show_all();

    State::register(window_id, State::new(window, main_view, connection, rx));
}
