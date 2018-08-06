use std::io::{self, BufReader, BufWriter, LineWriter, Write};
use std::thread;
use tokio_core::reactor::Handle;

use futures::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures::{Async, Future, Poll, Stream};
use serde_json;
use tokio::io::{lines, AsyncRead, Lines, ReadHalf, WriteHalf};

use remote::protocol::Object;
use remote::{Error, Result, ServerConnection, Session};

pub struct Client {
    lines: Lines<BufReader<ReadHalf<ServerConnection>>>,
    conn: LineWriter<BufWriter<WriteHalf<ServerConnection>>>,
    exit_pending: bool,
    events: UnboundedSender<Object>,
    requests: UnboundedReceiver<Object>,
}

impl Client {
    pub fn new(
        session: &Session,
        events: UnboundedSender<Object>,
        requests: UnboundedReceiver<Object>,
    ) -> Result<Client> {
        let stream = ServerConnection::new(&session.mode)?;
        let (stream_r, stream_w) = stream.split();
        let reader = BufReader::new(stream_r);
        let writer = BufWriter::new(stream_w);
        Ok(Client {
            lines: lines(reader),
            conn: LineWriter::new(writer),
            exit_pending: false,
            events,
            requests,
        })
    }

    fn send_request(&mut self, message: &Object) -> Result<()> {
        let json = serde_json::to_value(message.inner())?;
        let payload = serde_json::to_string(&json)? + "\n";
        self.conn.write_all(payload.as_bytes())?;
        Ok(())
    }

    fn handle_error(&self, error: &Error, line: &str) {
        error!("{}: {}", error, line);
    }
}

impl Future for Client {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match self.lines.poll().unwrap() {
                Async::Ready(Some(line)) => match line.parse() {
                    Ok(msg) => self.events.unbounded_send(msg).unwrap(),
                    Err(e) => self.handle_error(&e.into(), &line),
                },
                Async::Ready(None) => self.exit_pending = true,
                Async::NotReady => break,
            }
        }

        while !self.exit_pending {
            match self.requests.poll()? {
                Async::Ready(Some(request)) => self.send_request(&request).unwrap(),
                Async::Ready(None) => self.exit_pending = true,
                Async::NotReady => break,
            }
        }

        if self.exit_pending {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
    }
}

pub struct StdioClient {
    inner: Client,
}

impl StdioClient {
    pub fn new(handle: &Handle, session: &Session) -> Result<StdioClient> {
        let (events_tx, events_rx) = mpsc::unbounded();
        handle.spawn(events_rx.for_each(|e| {
            println!("{}", e);
            Ok(())
        }));
        let (requests_tx, requests_rx) = mpsc::unbounded();
        thread::spawn(move || {
            let stdin = io::stdin();
            let mut line = String::new();
            while let Ok(n) = stdin.read_line(&mut line) {
                if n == 0 {
                    break;
                }
                match line.parse() {
                    Ok(msg) => requests_tx.unbounded_send(msg).unwrap(),
                    Err(e) => error!("invalid message: {}: {}", e, line),
                }
                line.clear();
            }
        });
        Ok(StdioClient {
            inner: Client::new(session, events_tx, requests_rx)?,
        })
    }
}

impl Future for StdioClient {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.inner.poll()
    }
}
