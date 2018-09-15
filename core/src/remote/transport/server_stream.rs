use std::io;
use std::net::TcpStream;

use failure::Error;
use futures::{Future, Poll};
#[cfg(unix)]
use remote::transport::socket_unix::SocketStream;
#[cfg(windows)]
use remote::transport::socket_win::SocketStream;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream as TkTcpStream;
#[cfg(unix)]
use tokio::net::UnixStream as TkSocketStream;
#[cfg(windows)]
use tokio_named_pipes::NamedPipeStream as TkSocketStream;

use remote::transport::Stream;
use remote::ConnectionMode;

pub enum ServerStream {
    Socket(TkSocketStream),
    Tcp(TkTcpStream),
}

impl ServerStream {
    pub fn new(mode: &ConnectionMode) -> io::Result<ServerStream> {
        use self::ConnectionMode::*;
        match mode {
            Socket(path) => Ok(ServerStream::Socket(TkSocketStream::connect(path).wait()?)),
            Tcp(sock_addr) => Ok(ServerStream::Tcp(TkTcpStream::connect(sock_addr).wait()?)),
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

pub enum ServerStream2 {
    Socket(SocketStream),
    Tcp(TcpStream),
}

impl ServerStream2 {
    pub fn new(mode: &ConnectionMode) -> io::Result<ServerStream2> {
        use self::ConnectionMode::*;
        match mode {
            Socket(path) => Ok(ServerStream2::Socket(SocketStream::connect(path)?)),
            Tcp(sock_addr) => Ok(ServerStream2::Tcp(TcpStream::connect(sock_addr)?)),
        }
    }

    pub fn inner_clone(&self) -> Result<Box<Stream>, Error> {
        use self::ServerStream2::*;
        match self {
            Socket(inner) => {
                let cloned = inner.try_clone()?;
                Ok(Box::new(cloned))
            }
            Tcp(inner) => {
                let cloned = inner.try_clone()?;
                Ok(Box::new(cloned))
            }
        }
    }
}

impl io::Read for ServerStream2 {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use self::ServerStream2::*;
        match self {
            Socket(stream) => stream.try_clone()?.read(buf),
            Tcp(stream) => stream.read(buf),
        }
    }
}

impl io::Write for ServerStream2 {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use self::ServerStream2::*;
        match self {
            Socket(stream) => stream.try_clone()?.write(buf),
            Tcp(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        use self::ServerStream2::*;
        match self {
            Socket(stream) => stream.try_clone()?.flush(),
            Tcp(stream) => stream.flush(),
        }
    }
}
