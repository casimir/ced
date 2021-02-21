use std::io::BufRead;
use std::thread;

use crate::editor::Editor;
use futures_lite::*;
use remote::{protocol::requests::EditParams, Client, Request, Session};
use smol::{
    channel::{bounded, Sender},
    LocalExecutor, Unblock,
};

const CLIENT_ID: usize = 1;

pub fn start_standalone(filenames: &[&str]) {
    let (tx, rx) = bounded(100);
    let mut editor = Editor::new("", tx);

    editor.add_client(CLIENT_ID);
    for fname in filenames {
        let params = EditParams {
            name: fname.to_string(),
            scratch: false,
        };
        let _ = editor
            .command_edit(CLIENT_ID, &params)
            .map_err(|err| log::error!("could not open file '{}': {}", fname, err));
    }
    let rx = rx.clone();
    thread::spawn(move || loop {
        if let Ok(bm) = future::block_on(rx.recv()) {
            if bm.should_notify(CLIENT_ID) {
                println!("{}", &bm.message);
            }
        }
    });

    let stdin = std::io::stdin();
    for maybe_line in stdin.lock().lines() {
        match maybe_line {
            Ok(line) => match editor.handle(CLIENT_ID, &line) {
                Ok(response) => println!("{}", &response),
                Err(e) => eprintln!("{}: {:?}", e, line),
            },
            Err(e) => eprintln!("failed to read line: {}", e),
        }
    }

    editor.remove_client(CLIENT_ID);
}

pub struct StdioClient {
    client: Client,
    requests: Sender<Request>,
}

impl StdioClient {
    pub fn new(session: Session) -> io::Result<StdioClient> {
        let (client, requests) = Client::new(session)?;
        Ok(StdioClient { client, requests })
    }

    pub fn run(&self) -> io::Result<()> {
        let ex = LocalExecutor::new();
        future::block_on(ex.run(async {
            let stdin = Unblock::new(std::io::stdin());
            let mut stdout = Unblock::new(std::io::stdout());
            let requests_tx = self.requests.clone();
            ex.spawn(async move {
                let mut lines = io::BufReader::new(stdin).lines();
                while let Some(maybe_line) = lines.next().await {
                    match maybe_line {
                        Ok(line) => match line.parse() {
                            Ok(msg) => requests_tx.send(msg).await.expect("send request"),
                            Err(e) => log::error!("invalid message: {}: {}", e, line),
                        },
                        Err(e) => log::error!("failed to read line from stdin: {}", e),
                    }
                }
            })
            .detach();
            let (mut events, request_loop) = self.client.run().await?;
            ex.spawn(request_loop).detach();
            while let Some(event) = events.next().await {
                match event {
                    Ok(msg) => stdout.write_all(msg.to_string().as_bytes()).await?,
                    Err(e) => log::error!("invalid event: {}", e),
                }
            }
            Ok(())
        }))
    }
}
