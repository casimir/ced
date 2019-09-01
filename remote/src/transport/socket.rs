use std::fs;
use std::io;
use std::ops::Deref;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::Path;

use mio::{Evented, Poll, PollOpt, Ready, Token};
#[cfg(unix)]
use mio_uds::UnixListener;
#[cfg(windows)]
use mio_uds_windows::net::UnixStream;
#[cfg(windows)]
use mio_uds_windows::UnixListener;

pub struct SocketListener(UnixListener);

impl SocketListener {
    pub fn bind(path: &Path) -> io::Result<SocketListener> {
        let root_dir = path.parent().unwrap();
        if !root_dir.exists() {
            fs::create_dir_all(root_dir)?
        }
        Ok(SocketListener(UnixListener::bind(path)?))
    }
}

impl Evented for SocketListener {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.0.register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.0.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.0.deregister(poll)
    }
}

impl Deref for SocketListener {
    type Target = UnixListener;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct SocketStream(UnixStream);

impl SocketStream {
    pub fn connect(path: &Path) -> io::Result<SocketStream> {
        Ok(SocketStream(UnixStream::connect(path)?))
    }
}

impl Deref for SocketStream {
    type Target = UnixStream;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
