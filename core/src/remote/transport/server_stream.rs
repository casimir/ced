use std::io;

use futures::{Future, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream as SocketStream;
#[cfg(windows)]
use tokio_named_pipes::NamedPipeStream as SocketStream;

use remote::ConnectionMode;

pub enum ServerStream {
    Socket(SocketStream),
    Tcp(TcpStream),
}

impl ServerStream {
    pub fn new(mode: &ConnectionMode) -> io::Result<ServerStream> {
        use self::ConnectionMode::*;
        match mode {
            Socket(path) => Ok(ServerStream::Socket(SocketStream::connect(path).wait()?)),
            Tcp(sock_addr) => Ok(ServerStream::Tcp(TcpStream::connect(sock_addr).wait()?)),
        }
    }
}

impl io::Read for ServerStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use self::ServerStream::*;
        match self {
            Socket(stream) => stream.read(buf),
            Tcp(stream) => stream.read(buf),
        }
    }
}

impl AsyncRead for ServerStream {}

impl io::Write for ServerStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use self::ServerStream::*;
        match self {
            Socket(stream) => stream.write(buf),
            Tcp(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        use self::ServerStream::*;
        match self {
            Socket(stream) => stream.flush(),
            Tcp(stream) => stream.flush(),
        }
    }
}

impl AsyncWrite for ServerStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        use self::ServerStream::*;
        match self {
            Socket(stream) => stream.shutdown(),
            Tcp(stream) => stream.shutdown(),
        }
    }
}
