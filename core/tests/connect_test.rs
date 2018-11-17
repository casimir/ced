extern crate ced;
extern crate crossbeam_channel as channel;
extern crate failure;
extern crate itertools;

use std::thread::sleep;
use std::time::Duration;

use itertools::Itertools;

use ced::editor::Editor;
use ced::remote::jsonrpc::ClientEvent;
use ced::remote::protocol::notification::view::{Params as View, ParamsItem as ViewItem};
use ced::remote::{start_daemon, Client, Events, Session};
use ced::server::Broadcaster;

const CLIENT_ID: usize = 1;

#[derive(Clone, Default)]
struct State {
    view: View,
}

#[test]
fn starting_notifications() {
    let broadcaster = Broadcaster::default();
    let mut editor = Editor::new("", broadcaster.tx);
    let mut state = State::default();
    editor.add_client(CLIENT_ID);
    sleep(Duration::from_millis(150));
    while let Some(bm) = broadcaster.rx.try_recv() {
        match bm.message.method.as_str() {
            "view" => state.view = bm.message.params().unwrap().unwrap(),
            _ => {}
        }
    }
    editor.remove_client(CLIENT_ID);

    let view = state.view;
    let buffers: Vec<String> = view
        .iter()
        .filter_map(|item| match item {
            ViewItem::Header(header) => Some(header.buffer.clone()),
            _ => None,
        }).collect();
    assert_eq!(
        buffers,
        vec!["*debug*".to_string(), "*scratch*".to_string()]
    );
}

struct SyncClient {
    events: Events,
    state: State,
}

impl SyncClient {
    pub fn start(session: &Session) -> Result<SyncClient, failure::Error> {
        let (client, _) = Client::new(session)?;
        Ok(SyncClient {
            events: client.run(),
            state: State::default(),
        })
    }

    fn drain_notifications(&mut self) -> Result<(), failure::Error> {
        let is_notification = |res: &Result<ClientEvent, failure::Error>| match res {
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
        Ok(())
    }
}

fn start_client_and_server(session: &Session) -> SyncClient {
    let mut test_exe = std::env::current_exe().unwrap();
    test_exe.pop();
    test_exe.pop();
    test_exe.push("ced");
    start_daemon(test_exe.to_str().unwrap(), &session).expect("start a daemon");
    SyncClient::start(&session).unwrap()
}

#[cfg(unix)]
#[test]
fn connect_socket() {
    let session = Session::from_name("_test");
    let mut client = start_client_and_server(&session);
    client.drain_notifications().unwrap();

    let view = client.state.view;
    let buffers: Vec<String> = view
        .iter()
        .filter_map(|item| match item {
            ViewItem::Header(header) => Some(header.buffer.clone()),
            _ => None,
        }).collect();
    assert_eq!(
        buffers,
        vec!["*debug*".to_string(), "*scratch*".to_string()]
    );
}

#[test]
fn connect_tcp() {
    let session = Session::from_name("@:7357");
    let mut client = start_client_and_server(&session);
    client.drain_notifications().unwrap();

    let view = client.state.view;
    let buffers: Vec<String> = view
        .iter()
        .filter_map(|item| match item {
            ViewItem::Header(header) => Some(header.buffer.clone()),
            _ => None,
        }).collect();
    assert_eq!(
        buffers,
        vec!["*debug*".to_string(), "*scratch*".to_string()]
    );
}
