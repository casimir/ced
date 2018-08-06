mod client;
pub mod protocol;
mod session;
mod transport;

use std::io::{self, BufRead, BufReader, Write};
use std::thread;

use failure::Error;
use tokio_core::reactor::Core;

pub use self::client::{Client, StdioClient};
pub use self::session::{ConnectionMode, Session};
pub use self::transport::{Connection, EventedStream, Listener, ServerConnection};

#[derive(Debug, Fail)]
pub enum RemoteError {
    #[fail(display = "communication error: {}", err)]
    Communication { err: io::Error },
}

pub fn start_client(session: &Session) -> Result<(), Error> {
    let mut core = Core::new()?;
    let handle = core.handle();
    let client = StdioClient::new(&handle, session)?;
    core.run(client).expect("failed to start reactor");
    Ok(())
}

pub fn connect(
    session: &Session,
    input: &mut BufRead,
    mut output: Box<(Write + Send + Sync)>,
) -> Result<(), Error> {
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
