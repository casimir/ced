use std::io;

use mio::net::TcpListener;
use mio::Evented;

use crate::transport::socket::SocketListener;
use crate::transport::EventedStream;
use crate::{ConnectionMode, Session};

pub enum ServerListener {
    Socket(SocketListener),
    Tcp(TcpListener),
}

impl ServerListener {
    pub fn new(session: &Session) -> io::Result<ServerListener> {
        match &session.mode {
            ConnectionMode::Socket(path) => Ok(ServerListener::Socket(SocketListener::bind(path)?)),
            ConnectionMode::Tcp(sock_addr) => {
                Ok(ServerListener::Tcp(TcpListener::bind(&sock_addr)?))
            }
        }
    }

    pub fn inner(&self) -> &dyn Evented {
        use self::ServerListener::*;
        match self {
            Socket(inner) => inner,
            Tcp(inner) => inner,
        }
    }

    pub fn accept(&self) -> io::Result<Box<dyn EventedStream>> {
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
