use std::io::{self, BufRead, BufReader, BufWriter, LineWriter, Write};
use std::thread;

use crossbeam_channel as channel;
use failure::Error;
use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::{Async, Future, Poll, Stream};
use tokio::io::{lines, AsyncRead, Lines, ReadHalf, WriteHalf};

use remote::protocol::Object;
use remote::{ServerStream, ServerStream2, Session};

pub struct Client {
    lines: Lines<BufReader<ReadHalf<ServerStream>>>,
    conn: LineWriter<BufWriter<WriteHalf<ServerStream>>>,
    exit_pending: bool,
    events: UnboundedSender<Object>,
    requests: UnboundedReceiver<Object>,
}

impl Client {
    pub fn new(
        session: &Session,
        events: UnboundedSender<Object>,
        requests: UnboundedReceiver<Object>,
    ) -> Result<Client, Error> {
        let stream = ServerStream::new(&session.mode)?;
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

    fn send_request(&mut self, message: &Object) -> Result<(), io::Error> {
        self.conn.write_fmt(format_args!("{}\n", message))
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

pub struct Client2 {
    stream: ServerStream2,
    requests: channel::Receiver<Object>,
}

impl Client2 {
    pub fn new(session: &Session) -> Result<(Client2, channel::Sender<Object>), Error> {
        let (requests_tx, requests) = channel::unbounded();
        let client = Client2 {
            stream: ServerStream2::new(&session.mode)?,
            requests,
        };
        Ok((client, requests_tx))
    }

    pub fn run(&self) -> impl Iterator<Item = Result<Object, Error>> {
        let requests_rx = self.requests.clone();
        let mut writer = self.stream.inner_clone().expect("clone server stream");
        thread::spawn(move || {
            for message in requests_rx {
                writer.write_fmt(format_args!("{}\n", message)).unwrap();
            }
        });
        let reader = BufReader::new(self.stream.inner_clone().expect("clone server stream"));
        reader
            .lines()
            .map(|l| l.unwrap().parse().map_err(Error::from))
    }
}

pub struct StdioClient {
    client: Client2,
    requests: channel::Sender<Object>,
}

impl StdioClient {
    pub fn new(session: &Session) -> Result<StdioClient, Error> {
        let (client, requests) = Client2::new(session)?;
        Ok(StdioClient { client, requests })
    }

    pub fn run(&self) -> Result<(), Error> {
        let requests_tx = self.requests.clone();
        thread::spawn(move || {
            let stdin = io::stdin();
            for maybe_line in stdin.lock().lines() {
                match maybe_line {
                    Ok(line) => match line.parse() {
                        Ok(msg) => requests_tx.send(msg),
                        Err(e) => error!("invalid message: {}: {}", e, line),
                    },
                    Err(e) => error!("failed to read line from stdin: {}", e),
                }
            }
        });
        for event in self.client.run() {
            match event {
                Ok(msg) => println!("{}", msg),
                Err(e) => error!("invalid event: {}", e),
            }
        }
        Ok(())
    }
}
