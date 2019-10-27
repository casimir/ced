#[macro_use]
extern crate log;

mod components;
mod state;

use ced_remote::ConnectionEvent;
use gio::{ActionMapExt, ApplicationExt, ApplicationExtManual, MenuExt, SimpleActionExt};
use gtk::{GtkApplicationExt, GtkWindowExt, ObjectExt};

use self::components::show_session_chooser;
use self::state::State;

fn main() {
    env_logger::init();

    let application = gtk::Application::new("net.casimir-lab.ced", gio::ApplicationFlags::empty())
        .expect("initialize GTK");

    // replace with Glib::channel -> https://github.com/gtk-rs/examples/pull/222
    gtk::timeout_add(10, || {
        for window_id in State::ids() {
            State::with(window_id, |state, _| {
                while let Ok(ev) = state.events.try_recv() {
                    info!("message (window {})", window_id);
                    debug!("{:?}", ev);
                    match ev {
                        ConnectionEvent::Echo(_) => {
                            // TODO
                        }
                        ConnectionEvent::Info(client, session) => {
                            let title = format!("Window {} [{}@{}]", window_id, client, session);
                            state.window.set_title(&title);
                        }
                        ConnectionEvent::Menu(menu) => {
                            state.refresh_command_palette(&menu, window_id)
                        }
                        ConnectionEvent::Status(_) => {
                            // TODO
                        }
                        ConnectionEvent::View(view) => state.refresh_view(&view),
                    }
                }
            });
        }
        gtk::Continue(true)
    });

    application.connect_startup(|app| {
        let application = app;

        let menu = gio::Menu::new();
        menu.append("Quit", "app.quit");
        application.set_app_menu(&menu);
        application.set_accels_for_action("app.quit", &["<Primary>Q"]);

        let quit = gio::SimpleAction::new("quit", None);
        let wk_app = application.downgrade();
        quit.connect_activate(move |_, _| {
            wk_app.upgrade().map(|app| {
                app.quit();
            });
        });
        application.add_action(&quit);

        show_session_chooser(app);
    });
    application.connect_activate(|_| {});

    glib::set_application_name("ced");
    glib::set_prgname(Some("ced"));
    application.run(&std::env::args().collect::<Vec<_>>());
}
