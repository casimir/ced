#![cfg(windows)]

use std::net::Shutdown;
use std::os::windows::io::{AsRawSocket, RawSocket};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_io::Async;
use futures_lite::*;
use uds_windows::SocketAddr;

#[derive(Debug)]
struct AsyncUnixListener(Async<uds_windows::UnixListener>);

impl AsyncUnixListener {
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<AsyncUnixListener> {
        let path = path.as_ref().to_owned();
        Ok(AsyncUnixListener(Async::new(
            uds_windows::UnixListener::bind(path)?,
        )?))
    }

    pub async fn accept(&self) -> io::Result<(AsyncUnixStream, SocketAddr)> {
        let (stream, addr) = self.0.read_with(|io| io.accept()).await?;
        Ok((AsyncUnixStream(Arc::new(Async::new(stream)?)), addr))
    }

    pub fn get_ref(&self) -> &uds_windows::UnixListener {
        self.0.get_ref()
    }

    pub fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

#[derive(Clone, Debug)]
pub struct UnixListener(Arc<AsyncUnixListener>);

impl UnixListener {
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixListener> {
        let path = path.as_ref().to_owned();
        let listener = AsyncUnixListener::bind(path)?;
        Ok(UnixListener(Arc::new(listener)))
    }

    pub async fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        let (stream, addr) = self.0.accept().await?;
        Ok((UnixStream(Arc::new(stream)), addr))
    }

    pub fn incoming(&self) -> Incoming<'_> {
        Incoming(self)
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().local_addr()
    }
}

impl AsRawSocket for UnixListener {
    fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

#[derive(Debug)]
pub struct Incoming<'a>(&'a UnixListener);

impl Stream for Incoming<'_> {
    type Item = io::Result<UnixStream>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let future = self.0.accept();
        pin!(future);

        let (socket, _) = ready!(future.poll(cx))?;
        Poll::Ready(Some(Ok(socket)))
    }
}

#[derive(Debug)]
struct AsyncUnixStream(Arc<Async<uds_windows::UnixStream>>);

impl AsyncUnixStream {
    pub async fn connect<P: AsRef<Path>>(path: P) -> io::Result<AsyncUnixStream> {
        let stream = Async::new(uds_windows::UnixStream::connect(path)?)?;
        Ok(AsyncUnixStream(Arc::new(stream)))
    }

    pub fn pair() -> io::Result<(AsyncUnixStream, AsyncUnixStream)> {
        let (stream1, stream2) = uds_windows::UnixStream::pair()?;
        Ok((
            AsyncUnixStream(Arc::new(Async::new(stream1)?)),
            AsyncUnixStream(Arc::new(Async::new(stream2)?)),
        ))
    }

    pub fn get_ref(&self) -> &uds_windows::UnixStream {
        self.0.get_ref()
    }

    pub fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

impl Clone for AsyncUnixStream {
    fn clone(&self) -> Self {
        let inner = self.get_ref().try_clone().expect("clone stream");
        AsyncUnixStream(Arc::new(Async::new(inner).expect("async wrapper")))
    }
}

impl AsyncRead for AsyncUnixStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_read(cx, buf)
    }
}

impl AsyncRead for &AsyncUnixStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for AsyncUnixStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self).poll_close(cx)
    }
}

impl AsyncWrite for &AsyncUnixStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self.0).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self.0).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self.0).poll_close(cx)
    }
}

#[derive(Clone, Debug)]
pub struct UnixStream(Arc<AsyncUnixStream>);

impl UnixStream {
    pub async fn connect<P: AsRef<Path>>(path: P) -> io::Result<UnixStream> {
        let path = path.as_ref().to_owned();
        let stream = Arc::new(AsyncUnixStream::connect(path).await?);
        Ok(UnixStream(stream))
    }

    pub fn pair() -> io::Result<(UnixStream, UnixStream)> {
        let (a, b) = AsyncUnixStream::pair()?;
        let a = UnixStream(Arc::new(a));
        let b = UnixStream(Arc::new(b));
        Ok((a, b))
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().local_addr()
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().peer_addr()
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.get_ref().shutdown(how)
    }
}

impl AsRawSocket for UnixStream {
    fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

impl AsyncRead for UnixStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_read(cx, buf)
    }
}

impl AsyncRead for &UnixStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for UnixStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self).poll_close(cx)
    }
}

impl AsyncWrite for &UnixStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self.0).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self.0).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self.0).poll_close(cx)
    }
}
