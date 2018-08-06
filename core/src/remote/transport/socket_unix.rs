#![cfg(unix)]

use std::fs;
use std::os::unix::net::UnixStream;

use failure::Error;
use mio_uds::UnixListener;

use remote::{ConnectionMode, RemoteError, Session};

pub type SocketStream = UnixStream;

pub fn get_socket_stream(session: &Session) -> Result<SocketStream, RemoteError> {
    if let ConnectionMode::Socket(path) = &session.mode {
        match UnixStream::connect(path) {
            Ok(s) => Ok(s),
            Err(err) => Err(RemoteError::Communication { err }),
        }
    } else {
        unreachable!();
    }
}

pub type SocketListener = UnixListener;

pub fn get_socket_listener(session: &Session) -> Result<SocketListener, Error> {
    if let ConnectionMode::Socket(path) = &session.mode {
        let root_dir = path.parent().unwrap();
        if !root_dir.exists() {
            fs::create_dir_all(root_dir)?
        }
        Ok(UnixListener::bind(path)?)
    } else {
        unreachable!()
    }
}
