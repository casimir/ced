mod helpers;

use std::env;
use std::io;

use ced::editor::{BUFFER_DEBUG, BUFFER_SCRATCH};
use ced::remote::jsonrpc::ClientEvent;
use ced::remote::protocol::notifications::ViewParams;
use ced::remote::{start_daemon, Client, ClientEventStream, Session};
use futures_lite::*;

const CLIENT_ID: usize = 1;

fn assert_view_state(view: &ViewParams) {
    let buffers: Vec<String> = view.iter().map(|item| item.buffer.to_owned()).collect();
    assert_eq!(
        buffers,
        vec![BUFFER_DEBUG.to_owned(), BUFFER_SCRATCH.to_owned()]
    );
}

#[test]
fn starting_notifications() {
    let mut editor = helpers::SequentialEditor::new();
    editor.add_client(CLIENT_ID);
    editor.step();
    editor.remove_client(CLIENT_ID);

    let view = &editor.state().view;
    assert_view_state(view);
}

#[derive(Clone, Default)]
struct State {
    view: ViewParams,
}

struct SyncClient {
    events: ClientEventStream,
    state: State,
}

impl SyncClient {
    pub async fn start(session: Session) -> io::Result<SyncClient> {
        let (client, _) = Client::new(session);
        let (events, _) = client.run().await?;
        Ok(SyncClient {
            events,
            state: State::default(),
        })
    }

    async fn drain_notifications(&mut self) {
        let is_notification = |res: &Result<ClientEvent, _>| match res {
            Ok(ev) => ev.is_notification(),
            _ => false,
        };
        // FIXME deadlock with async-io > 1.1.3
        log::trace!("starting to drain queued notifications");
        while let Some(ev) = self.events.next().await {
            log::trace!("checking event: {:?}", ev);
            if !is_notification(&ev) {
                break;
            }
            if let ClientEvent::Notification(noti) = ev.unwrap() {
                if let "view" = noti.method.as_str() {
                    self.state.view = noti.params().unwrap().unwrap();
                    break; // FIXME don't stop on special case
                }
            }
        }
    }
}

async fn start_client_and_server(session: Session) -> SyncClient {
    if env::var("CED_BIN").is_err() {
        let mut test_exe = std::env::current_exe().unwrap();
        test_exe.pop();
        test_exe.pop();
        test_exe.push("ced");
        env::set_var("CED_BIN", test_exe.display().to_string());
    }
    start_daemon(&session).expect("start the daemon");
    SyncClient::start(session).await.expect("start the client")
}

#[test]
fn connect_socket() {
    let session = Session::from_name("_test");
    future::block_on(async {
        let mut client = start_client_and_server(session).await;
        client.drain_notifications().await;
        assert_view_state(&client.state.view);
    });
}

#[test]
fn connect_tcp() {
    let _ = env_logger::builder().is_test(true).try_init();
    let session = Session::from_name("@:7357");
    future::block_on(async {
        let mut client = start_client_and_server(session).await;
        client.drain_notifications().await;
        assert_view_state(&client.state.view);
    });
}
