use crate::jsonrpc::{ClientEvent, JsonCodingError, Request};
use crate::session::Session;
use crate::transport::ServerStream;
use async_channel::{unbounded, Receiver, Sender};
use futures_lite::*;

pub type ClientEventResult = Result<ClientEvent, JsonCodingError>;

pub type ClientEventStream = stream::Map<
    io::Lines<io::BufReader<ServerStream>>,
    fn(io::Result<String>) -> ClientEventResult,
>;

pub struct Client {
    session: Session,
    requests: Receiver<Request>,
}

impl Client {
    pub fn new(session: Session) -> io::Result<(Client, Sender<Request>)> {
        let (requests_tx, requests) = unbounded();
        let client = Client { session, requests };
        Ok((client, requests_tx))
    }

    pub async fn run(&self) -> io::Result<(ClientEventStream, impl Future<Output = ()>)> {
        let mut requests_rx = self.requests.clone();
        let stream = ServerStream::new(&self.session.mode).await?;
        let mut writer = stream.clone();
        let request_loop = async move {
            while let Some(message) = requests_rx.next().await {
                // TODO error
                let _ = writer.write_all(format!("{}\n", message).as_bytes()).await;
            }
        };
        fn parse_line(x: io::Result<String>) -> Result<ClientEvent, JsonCodingError> {
            // TODO handle io::Error (eg: closed connection)
            x.expect("parse line").parse()
        }
        Ok((
            io::BufReader::new(stream).lines().map(parse_line),
            request_loop,
        ))
    }
}
