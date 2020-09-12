use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_net::TcpListener;
use futures_lite::*;

use crate::transport::socket::SocketListener;
use crate::transport::ServerStream;
use crate::{ConnectionMode, Session};

#[derive(Debug)]
pub enum ServerListener {
    Socket(SocketListener),
    Tcp(TcpListener),
}

impl ServerListener {
    pub async fn bind(session: &Session) -> io::Result<ServerListener> {
        match &session.mode {
            ConnectionMode::Socket(path) => {
                Ok(ServerListener::Socket(SocketListener::bind(path).await?))
            }
            ConnectionMode::Tcp(sock_addr) => {
                Ok(ServerListener::Tcp(TcpListener::bind(&sock_addr).await?))
            }
        }
    }

    pub async fn accept(&self) -> io::Result<ServerStream> {
        use self::ServerListener::*;
        match self {
            Socket(inner) => {
                let (stream, _) = inner.accept().await?;
                Ok(ServerStream::Socket(stream))
            }
            Tcp(inner) => {
                let (stream, _) = inner.accept().await?;
                Ok(ServerStream::Tcp(stream))
            }
        }
    }
}

#[derive(Debug)]
pub struct Incoming<'a>(&'a ServerListener);

impl<'a> Stream for Incoming<'a> {
    type Item = io::Result<ServerStream>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let future = self.0.accept();
        pin!(future);

        let socket = ready!(future.poll(cx))?;
        Poll::Ready(Some(Ok(socket)))
    }
}

impl ServerListener {
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming(self)
    }
}
