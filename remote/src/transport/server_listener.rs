use std::io;

use failure::Error;
use mio::net::TcpListener;
use mio::Evented;

use transport::EventedStream;
use {ConnectionMode, Session};

use transport::socket::SocketListener;

pub enum ServerListener {
    Socket(SocketListener),
    Tcp(TcpListener),
}

impl ServerListener {
    pub fn new(session: &Session) -> Result<ServerListener, Error> {
        match &session.mode {
            ConnectionMode::Socket(path) => Ok(ServerListener::Socket(SocketListener::bind(path)?)),
            ConnectionMode::Tcp(sock_addr) => {
                Ok(ServerListener::Tcp(TcpListener::bind(&sock_addr)?))
            }
        }
    }

    pub fn inner(&self) -> &Evented {
        use self::ServerListener::*;
        match self {
            Socket(inner) => inner,
            Tcp(inner) => inner,
        }
    }

    pub fn accept(&self) -> io::Result<Box<EventedStream>> {
        use self::ServerListener::*;
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
