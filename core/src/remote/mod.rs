mod client;
pub mod protocol;
mod session;
mod transport;

use std::io::{self, BufRead, BufReader, Write};
use std::thread;

use serde_json;

pub use self::client::Client;
pub use self::session::{ConnectionMode, Session};
pub use self::transport::{Connection, EventedStream, Listener};

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Communication(err: io::Error) {
            from()
            display("communication error: {}", err)
        }
        Protocol(err: serde_json::Error) {
            from()
            display("protocol error: {}", err)
        }
        Connection(err: ::std::net::AddrParseError) {
            from()
            display("invalid address: {}", err)
        }
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;

pub fn connect(
    session: &Session,
    input: &mut BufRead,
    mut output: Box<(Write + Send + Sync)>,
) -> Result<()> {
    let conn = Connection::new(&session)?;
    let inner_stream = conn.inner_clone()?;
    thread::spawn(move || {
        let mut reader = BufReader::new(inner_stream);
        let mut line = String::new();
        loop {
            reader
                .read_line(&mut line)
                .expect("failed to read server stream");
            if !line.is_empty() {
                if let Err(e) = output.write_all(line.as_bytes()) {
                    error!("write error: {}", e);
                };
                line.clear();
            }
        }
    });

    let mut stream = conn.inner_clone()?;
    let mut line = String::new();
    while let Ok(n) = input.read_line(&mut line) {
        if n == 0 {
            break;
        }
        if let Err(e) = stream.write_all(line.as_bytes()) {
            error!("write error: {}", e);
        };
        line.clear();
    }
    Ok(())
}
