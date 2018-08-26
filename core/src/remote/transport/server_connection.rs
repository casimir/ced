use std::io;

use futures::{Future, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream as SocketStream;
#[cfg(windows)]
use tokio_named_pipes::NamedPipeStream as SocketStream;

use remote::ConnectionMode;

pub enum ServerConnection {
    Socket(SocketStream),
    Tcp(TcpStream),
}

impl ServerConnection {
    pub fn new(mode: &ConnectionMode) -> io::Result<ServerConnection> {
        use self::ConnectionMode::*;
        match mode {
            Socket(path) => Ok(ServerConnection::Socket(
                SocketStream::connect(path).wait()?,
            )),
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
