extern crate ced_remote;
extern crate env_logger;
extern crate gtk;
#[macro_use]
extern crate log;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};

use ced_remote::protocol::notification::menu::Entry as MenuEntry;
use ced_remote::{ensure_session, ClientEvent, Connection, Session};
use gtk::prelude::*;
use gtk::{Button, Orientation, Window, WindowType};

struct State {
    window: Window,
    connection: Connection,
    events: Receiver<ClientEvent>,
}

thread_local! {
    static STATES: RefCell<HashMap<usize, State>> = RefCell::new(HashMap::new());
    static CHOOSER: RefCell<Option<Window>> = RefCell::new(None);
}

fn show_session_chooser() {
    info!("start session chooser");
    let window = Window::new(WindowType::Toplevel);
    window.set_title("Sessions chooser");
    window.set_default_size(350, 70);
    window.connect_delete_event(|_, _| {
        CHOOSER.with(|global| *global.borrow_mut() = None);
        STATES.with(|global| {
            if global.borrow().len() == 0 {
                gtk::main_quit();
            }
        });
        Inhibit(false)
    });

    let container = gtk::Box::new(Orientation::Vertical, 0);
    for session in Session::list() {
        let button = Button::new_with_label(&session);
        button.connect_clicked(|bttn| {
            let session = bttn.get_label().unwrap();
            show_new_client(Some(&session));
            CHOOSER.with(|global| {
                if let Some(ref win) = *global.borrow() {
                    win.close()
                }
            });
            CHOOSER.with(|global| *global.borrow_mut() = None);
        });
        container.pack_start(&button, true, true, 0);
    }
    let button = Button::new_with_label("+");
    button.connect_clicked(|_| {
        show_new_client(None);
        CHOOSER.with(|global| {
            if let Some(ref win) = *global.borrow() {
                win.close()
            }
        });
        CHOOSER.with(|global| *global.borrow_mut() = None);
    });
    container.pack_start(&button, true, true, 0);

    container.show_all();
    window.add(&container);
    window.show_all();
    CHOOSER.with(|global| *global.borrow_mut() = Some(window));
}

fn show_new_client(session_name: Option<&str>) {
    let session = match session_name {
        Some(name) => Session::from_name(name),
        None => Session::from_pid(),
    };
    info!("start new client for session '{}'", session);
    let connection = Connection::new(&session).expect("establish connection");
    let conn_events = connection.connect();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        info!("ensure session availability");
        ensure_session("ced", &session).expect("ensure session");
        info!("init connection to session");
        for ev in conn_events {
            tx.send(ev).expect("send event");
        }
    });

    let window_id = STATES.with(|global| {
        global
            .borrow()
            .keys()
            .fold(1, |curr, key| std::cmp::max(curr, *key))
            + 1
    });
    let window = Window::new(WindowType::Toplevel);
    window.set_default_size(350, 70);

    let button1 = Button::new_with_label("Click me!");
    let button2 = Button::new_with_label("Click me too!");

    let main = gtk::Box::new(Orientation::Vertical, 0);
    main.pack_start(&button1, true, true, 0);
    main.pack_start(&button2, true, true, 0);
    main.show_all();

    window.add(&main);
    window.show_all();

    window.connect_delete_event(move |_, _| {
        STATES.with(|global| {
            info!("delete window and client '{}'", window_id);
            let mut states = global.borrow_mut();
            let mut state = states.remove(&window_id).unwrap();
            state.connection.quit();
            if states.len() == 0 {
                show_session_chooser();
            }
        });
        Inhibit(false)
    });

    button1.connect_clicked(|_| {
        println!("Clicked!");
    });

    STATES.with(move |global| {
        global.borrow_mut().insert(
            window_id,
            State {
                window,
                connection,
                events: rx,
            },
        );
    });
}

fn main() {
    env_logger::init();

    gtk::init().expect("initialize GTK");
    gtk::timeout_add(10, || {
        STATES.with(|global| {
            for (window_id, state) in &*global.borrow() {
                while let Ok(ev) = state.events.try_recv() {
                    info!("message (window {})", window_id);
                    let ctx = state.connection.state();
                    println!("{}", ev);
                    println!("{:?}", ctx);
                    let title = format!("Window {} [{}]", window_id, ctx.session);
                    state.window.set_title(&title);
                }
            }
        });
        Continue(true)
    });
    show_session_chooser();
    gtk::main();
}
