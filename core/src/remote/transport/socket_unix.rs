#![cfg(unix)]

use std::fs;
use std::io;
use std::ops::Deref;
use std::path::Path;

use failure::Error;
use mio::{Evented, Poll, PollOpt, Ready, Token};
use mio_uds::UnixListener;

pub struct SocketListener(UnixListener);

impl SocketListener {
    pub fn bind(path: &Path) -> Result<SocketListener, Error> {
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
