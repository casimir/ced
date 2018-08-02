#![cfg(unix)]

use std::io::{self, BufReader, BufWriter, LineWriter, Write};

use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::{Async, Future, Poll, Stream};
use jsonrpc_lite::JsonRpc;
use serde_json;
use tokio::io::{lines, AsyncRead, AsyncWrite, Lines, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio_uds::UnixStream;

use remote::protocol::Object;
use remote::{ConnectionMode, Error, Result, Session};

enum ServerConnection {
    Socket(UnixStream),
    Tcp(TcpStream),
}

impl ServerConnection {
    fn new(mode: &ConnectionMode) -> io::Result<ServerConnection> {
        use self::ConnectionMode::*;
        match mode {
            Socket(path) => Ok(ServerConnection::Socket(UnixStream::connect(path).wait()?)),
            Tcp(sock_addr) => Ok(ServerConnection::Tcp(TcpStream::connect(sock_addr).wait()?)),
        }
    }
}

impl io::Read for ServerConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use self::ServerConnection::*;
        match self {
            Socket(stream) => stream.read(buf),
            Tcp(stream) => stream.read(buf),
        }
    }
}

impl AsyncRead for ServerConnection {}

impl io::Write for ServerConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use self::ServerConnection::*;
        match self {
            Socket(stream) => stream.write(buf),
            Tcp(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        use self::ServerConnection::*;
        match self {
            Socket(stream) => stream.flush(),
            Tcp(stream) => stream.flush(),
        }
    }
}

impl AsyncWrite for ServerConnection {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        use self::ServerConnection::*;
        match self {
            Socket(stream) => stream.shutdown(),
            Tcp(stream) => stream.shutdown(),
        }
    }
}

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
