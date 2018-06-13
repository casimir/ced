use std::io::Read;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::unix::net::UnixStream;
use std::thread;

use serde_json;

use server::{ServerMode, SessionManager};

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

trait Stream: Read + Write + Send {}

impl<T> Stream for T
where
    T: Read + Write + Send,
{
}

enum Connection {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
}

impl Connection {
    fn inner_clone(&self) -> Result<Box<Stream>> {
        use self::Connection::*;
        match self {
            Tcp(inner) => {
                let cloned = inner.try_clone()?;
                Ok(Box::new(cloned))
            }
            #[cfg(unix)]
            Unix(inner) => {
                let cloned = inner.try_clone()?;
                Ok(Box::new(cloned))
            }
        }
    }
}

fn get_connection(mode: &ServerMode) -> Result<Connection> {
    match mode {
        ServerMode::Tcp(addr) => {
            let sock_addr: SocketAddr = addr.parse()?;
            match TcpStream::connect(&sock_addr) {
                Ok(s) => Ok(Connection::Tcp(s)),
                Err(e) => Err(Error::Communication(e)),
            }
        }
        #[cfg(unix)]
        ServerMode::UnixSocket(name) => {
            let path = SessionManager::new().session_full_path(name);
            match UnixStream::connect(path) {
                Ok(s) => Ok(Connection::Unix(s)),
                Err(e) => Err(Error::Communication(e)),
            }
        }
        _ => unreachable!(),
    }
}

pub fn connect(input: &mut BufRead, mut output: Box<(Write + Send + Sync)>, mode: &ServerMode) {
    let conn = get_connection(mode).unwrap();
    let mut stream = conn.inner_clone().unwrap();
    let inner_stream = conn.inner_clone().unwrap();
    thread::spawn(move || {
        let mut reader = BufReader::new(inner_stream);
        let mut buf = String::new();
        while let Ok(n) = reader.read_line(&mut buf) {
            if n == 0 {
                break;
            }
            match output.write(buf.as_bytes()) {
                Err(e) => eprintln!("error: {}", e),
                _ => (),
            };
        }
    });

    let mut line = String::new();
    while let Ok(n) = input.read_line(&mut line) {
        if n == 0 {
            break;
        }
        match stream.write(line.as_bytes()) {
            Err(e) => eprintln!("error: {}", e),
            _ => (),
        };
        line.clear();
    }
}
