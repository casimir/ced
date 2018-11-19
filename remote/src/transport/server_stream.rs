use std::io;
use std::net::TcpStream;

use failure::Error;

use transport::socket::SocketStream;
use transport::Stream;
use ConnectionMode;

pub enum ServerStream {
    Socket(SocketStream),
    Tcp(TcpStream),
}

impl ServerStream {
    pub fn new(mode: &ConnectionMode) -> io::Result<ServerStream> {
        use self::ConnectionMode::*;
        match mode {
            Socket(path) => Ok(ServerStream::Socket(SocketStream::connect(path)?)),
            Tcp(sock_addr) => Ok(ServerStream::Tcp(TcpStream::connect(sock_addr)?)),
        }
    }

    pub fn inner_clone(&self) -> Result<Box<Stream>, Error> {
        use self::ServerStream::*;
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

impl io::Read for ServerStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use self::ServerStream::*;
        match self {
            Socket(stream) => stream.try_clone()?.read(buf),
            Tcp(stream) => stream.read(buf),
        }
    }
}

impl io::Write for ServerStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use self::ServerStream::*;
        match self {
            Socket(stream) => stream.try_clone()?.write(buf),
            Tcp(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        use self::ServerStream::*;
        match self {
            Socket(stream) => stream.try_clone()?.flush(),
            Tcp(stream) => stream.flush(),
        }
    }
}
