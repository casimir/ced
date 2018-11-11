use std::io::{self, BufRead, BufReader, Lines, Write};
use std::thread;

use crossbeam_channel as channel;
use failure::Error;

use jsonrpc::{ClientEvent, Request};
use remote::{ServerStream, Session, Stream};

pub struct Events {
    lines: Lines<BufReader<Box<Stream>>>,
}

impl Iterator for Events {
    type Item = Result<ClientEvent, Error>;

    fn next(&mut self) -> Option<Result<ClientEvent, Error>> {
        self.lines
            .next()
            .map(|l| l.unwrap().parse().map_err(Error::from))
    }
}

pub struct Client {
    stream: ServerStream,
    requests: channel::Receiver<Request>,
}

impl Client {
    pub fn new(session: &Session) -> Result<(Client, channel::Sender<Request>), Error> {
        let (requests_tx, requests) = channel::unbounded();
        let client = Client {
            stream: ServerStream::new(&session.mode)?,
            requests,
        };
        Ok((client, requests_tx))
    }

    pub fn run(&self) -> Events {
        let requests_rx = self.requests.clone();
        let mut writer = self.stream.inner_clone().expect("clone server stream");
        thread::spawn(move || {
            for message in requests_rx {
                writer.write_fmt(format_args!("{}\n", message)).unwrap();
            }
        });
        let reader = BufReader::new(self.stream.inner_clone().expect("clone server stream"));
        Events {
            lines: reader.lines(),
        }
    }
}

pub struct StdioClient {
    client: Client,
    requests: channel::Sender<Request>,
}

impl StdioClient {
    pub fn new(session: &Session) -> Result<StdioClient, Error> {
        let (client, requests) = Client::new(session)?;
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
