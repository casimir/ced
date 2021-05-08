use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_net::TcpStream;
use futures_lite::*;

use crate::transport::socket::SocketStream;
use crate::ConnectionMode;

#[derive(Clone, Debug)]
pub enum ServerStream {
    Socket(SocketStream),
    Tcp(TcpStream),
}

impl ServerStream {
    pub async fn new(mode: &ConnectionMode) -> io::Result<ServerStream> {
        use self::ConnectionMode::*;
        match mode {
            Socket(path) => Ok(ServerStream::Socket(SocketStream::connect(path).await?)),
            Tcp(sock_addr) => Ok(ServerStream::Tcp(TcpStream::connect(sock_addr).await?)),
        }
    }
}

impl AsyncRead for ServerStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        log::trace!("polling for read");
        use ServerStream::*;
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Socket(inner) => Pin::new_unchecked(inner).poll_read(cx, buf),
                Tcp(inner) => Pin::new_unchecked(inner).poll_read(cx, buf),
            }
        }
    }
}

impl AsyncWrite for ServerStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        log::trace!("polling for write");
        use ServerStream::*;
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Socket(inner) => Pin::new_unchecked(inner).poll_write(cx, buf),
                Tcp(inner) => Pin::new_unchecked(inner).poll_write(cx, buf),
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        log::trace!("polling for flush");
        use ServerStream::*;
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Socket(inner) => Pin::new_unchecked(inner).poll_flush(cx),
                Tcp(inner) => Pin::new_unchecked(inner).poll_flush(cx),
            }
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        log::trace!("polling for close");
        use ServerStream::*;
        unsafe {
            match Pin::get_unchecked_mut(self) {
                Socket(inner) => Pin::new_unchecked(inner).poll_close(cx),
                Tcp(inner) => Pin::new_unchecked(inner).poll_close(cx),
            }
        }
    }
}
