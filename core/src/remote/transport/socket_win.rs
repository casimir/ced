#![cfg(windows)]

use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle};

use mio::{Evented, Poll, PollOpt, Ready, Token};
use mio_named_pipes::NamedPipe;
use winapi::um::winbase::FILE_FLAG_OVERLAPPED;

use remote::{ConnectionMode, Error, Result, Session};

pub type SocketStream = File;

pub fn get_socket_stream(session: &Session) -> Result<SocketStream> {
    if let ConnectionMode::Socket(path) = &session.mode {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(FILE_FLAG_OVERLAPPED)
            .open(path)?;
        Ok(file)
    } else {
        unreachable!()
    }
}

pub struct Socket(NamedPipe);

impl Socket {
    pub fn try_clone(&self) -> Result<Socket> {
        unsafe { Ok(Socket(NamedPipe::from_raw_handle(self.0.as_raw_handle()))) }
    }
}

impl io::Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> ::std::result::Result<usize, io::Error> {
        self.0.read(buf)
    }
}

impl io::Write for Socket {
    fn write(&mut self, buf: &[u8]) -> ::std::result::Result<usize, io::Error> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> ::std::result::Result<(), io::Error> {
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

// pub type SocketStream = Socket;

// pub fn get_socket_stream(session: &Session) -> Result<SocketStream> {
//     if let ConnectionMode::Socket(path) = &session.mode {
//         let file = OpenOptions::new()
//             .read(true)
//             .write(true)
//             .custom_flags(FILE_FLAG_OVERLAPPED)
//             .open(path)?;
//         let pipe = unsafe { NamedPipe::from_raw_handle(file.into_raw_handle()) };
//         loop {
//             match pipe.connect() {
//                 Ok(_) => break,
//                 Err(e) => {
//                     if e.kind() == ::std::io::ErrorKind::WouldBlock {
//                         continue;
//                     } else {
//                         return Err(Error::Communication(e));
//                     }
//                 }
//             }
//         }
//         Ok(Socket(pipe))
//     } else {
//         unreachable!()
//     }
// }

pub type SocketListener = Socket;

impl SocketListener {
    pub fn accept(&self) -> io::Result<Option<(Socket, ())>> {
        self.0.connect()?;
        Ok(Some((self.try_clone().unwrap(), ())))
    }
}

pub fn get_socket_listener(session: &Session) -> Result<SocketListener> {
    if let ConnectionMode::Socket(path) = &session.mode {
        Ok(Socket(NamedPipe::new(path)?))
    } else {
        unreachable!();
    }
}
