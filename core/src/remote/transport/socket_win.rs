#![cfg(windows)]

use std::fs::{File, OpenOptions};
use std::io;
use std::ops::Deref;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;

use failure::Error;
use mio::{Evented, Poll, PollOpt, Ready, Token};
use mio_named_pipes::NamedPipe;
use winapi::um::winbase::FILE_FLAG_OVERLAPPED;

pub struct Socket(NamedPipe);

impl Socket {
    fn new(path: &Path) -> Result<Socket, Error> {
        Ok(Socket(NamedPipe::new(path)?))
    }

    pub fn try_clone(&self) -> Result<Socket, Error> {
        unsafe { Ok(Socket(NamedPipe::from_raw_handle(self.0.as_raw_handle()))) }
    }

    pub fn accept(&self) -> io::Result<Option<(Socket, ())>> {
        self.0.connect()?;
        Ok(Some((self.try_clone().unwrap(), ())))
    }
}

impl Deref for Socket {
    type Target = NamedPipe;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl io::Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl io::Write for Socket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl Evented for Socket {
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

pub struct SocketListener(Socket);

impl SocketListener {
    pub fn bind(path: &Path) -> Result<SocketListener, Error> {
        Ok(SocketListener(Socket::new(path)?))
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
    type Target = Socket;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct SocketStream(File);

impl SocketStream {
    pub fn connect(path: &Path) -> io::Result<SocketStream> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(FILE_FLAG_OVERLAPPED)
            .open(path)?;
        Ok(SocketStream(file))
    }
}

impl Deref for SocketStream {
    type Target = File;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
