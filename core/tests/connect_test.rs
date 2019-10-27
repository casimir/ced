mod helpers;

use std::env;
use std::io;

use ced::editor::{BUFFER_DEBUG, BUFFER_SCRATCH};
use ced::remote::jsonrpc::ClientEvent;
use ced::remote::protocol::notifications::ViewParams;
use ced::remote::{start_daemon, Client, Events, Session};
use itertools::Itertools;

const CLIENT_ID: usize = 1;

#[test]
fn starting_notifications() {
    let mut editor = helpers::SequentialEditor::new();
    editor.add_client(CLIENT_ID);
    editor.step();
    editor.remove_client(CLIENT_ID);

    let view = &editor.state().view;
    let buffers: Vec<String> = view.iter().map(|item| item.buffer.to_owned()).collect();
    assert_eq!(
        buffers,
        vec![BUFFER_DEBUG.to_owned(), BUFFER_SCRATCH.to_owned()]
    );
}

#[derive(Clone, Default)]
struct State {
    view: ViewParams,
}

struct SyncClient {
    events: Events,
    state: State,
}

impl SyncClient {
    pub fn start(session: &Session) -> io::Result<SyncClient> {
        let (client, _) = Client::new(session)?;
        Ok(SyncClient {
            events: client.run(),
            state: State::default(),
        })
    }

    fn drain_notifications(&mut self) {
        let is_notification = |res: &Result<ClientEvent, _>| match res {
            Ok(ev) => ev.is_notification(),
            _ => false,
        };
        let mut peeker = self.events.by_ref().peekable();
        for ev in peeker.peeking_take_while(is_notification) {
            // FIXME should stop instead of blocking
            match ev.unwrap() {
                ClientEvent::Notification(noti) => match noti.method.as_str() {
                    "view" => {
                        self.state.view = noti.params().unwrap().unwrap();
                        break; // FIXME don't stop on special case
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

fn start_client_and_server(session: &Session) -> SyncClient {
    if env::var("CED_BIN").is_err() {
        let mut test_exe = std::env::current_exe().unwrap();
        test_exe.pop();
        test_exe.pop();
        test_exe.push("ced");
        env::set_var("CED_BIN", test_exe.display().to_string());
    }
    start_daemon(&session).expect("start the daemon");
    SyncClient::start(&session).expect("start the client")
}

// reactivate when a windows CI with 1803+ is available
#[cfg(unix)]
#[test]
fn connect_socket() {
    let session = Session::from_name("_test");
    let mut client = start_client_and_server(&session);
    client.drain_notifications();

    let view = client.state.view;
    let buffers: Vec<String> = view.iter().map(|item| item.buffer.to_owned()).collect();
    assert_eq!(
        buffers,
        vec![BUFFER_DEBUG.to_owned(), BUFFER_SCRATCH.to_owned()]
    );
}

#[test]
fn connect_tcp() {
    let session = Session::from_name("@:7357");
    let mut client = start_client_and_server(&session);
    client.drain_notifications();

    let view = client.state.view;
    let buffers: Vec<String> = view.iter().map(|item| item.buffer.to_owned()).collect();
    assert_eq!(
        buffers,
        vec![BUFFER_DEBUG.to_owned(), BUFFER_SCRATCH.to_owned()]
    );
}
