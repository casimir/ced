#![cfg(unix)]

use std::io::{BufReader, BufWriter, LineWriter, Write};

use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::{Async, Future, Poll, Stream};
use jsonrpc_lite::JsonRpc;
use serde_json;
use tokio::io::{lines, AsyncRead, Lines, ReadHalf, WriteHalf};
use tokio_uds::UnixStream;

use remote::protocol::Object;
use remote::{ConnectionMode, Error, Result, Session};

pub struct Client {
    lines: Lines<BufReader<ReadHalf<UnixStream>>>,
    conn: LineWriter<BufWriter<WriteHalf<UnixStream>>>,
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
        let stream = if let ConnectionMode::Socket(path) = &session.mode {
            UnixStream::connect(&path).wait()?
        } else {
            unreachable!();
        };
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
                Async::Ready(Some(line)) => match JsonRpc::parse(&line) {
                    Ok(message) => self.events.unbounded_send(message.into()).unwrap(),
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
