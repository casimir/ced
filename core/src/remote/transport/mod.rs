mod server_connection;
mod socket_unix;
mod socket_win;

use std::io;

use failure::Error;
use mio::net::TcpListener;
use mio::Evented;

pub use self::server_connection::ServerConnection;
#[cfg(unix)]
use self::socket_unix::{get_socket_listener, SocketListener};
#[cfg(windows)]
use self::socket_win::{get_socket_listener, SocketListener};

use remote::{ConnectionMode, Session};

pub trait Stream: io::Read + io::Write + Send {}

impl<T> Stream for T where T: io::Read + io::Write + Send {}

pub trait EventedStream: Stream + Evented {}

impl<T> EventedStream for T where T: Stream + Evented {}

pub enum Listener {
    Socket(SocketListener),
    Tcp(TcpListener),
}

impl Listener {
    pub fn new(session: &Session) -> Result<Listener, Error> {
        match &session.mode {
            ConnectionMode::Socket(_) => Ok(Listener::Socket(get_socket_listener(&session)?)),
            ConnectionMode::Tcp(sock_addr) => Ok(Listener::Tcp(TcpListener::bind(&sock_addr)?)),
        }
    }

    pub fn inner(&self) -> &Evented {
        use self::Listener::*;
        match self {
            Socket(inner) => inner,
            Tcp(inner) => inner,
        }
    }

    pub fn accept(&self) -> io::Result<Box<EventedStream>> {
        use self::Listener::*;
        match self {
            Socket(inner) => {
                let opt = inner.accept()?;
                // None when no connection is waiting to be accepted
                let (stream, _) = opt.unwrap();
                Ok(Box::new(stream))
            }
            Tcp(inner) => {
                let (stream, _) = inner.accept()?;
                Ok(Box::new(stream))
            }
        }
    }
}
