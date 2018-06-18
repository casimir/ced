#![cfg(unix)]

use std::fs;
use std::os::unix::net::UnixStream;

use mio_uds::UnixListener;

use remote::{ConnectionMode, Error, Result, Session};

pub type SocketStream = UnixStream;

pub fn get_socket_stream(session: &Session) -> Result<SocketStream> {
    if let ConnectionMode::Socket(path) = &session.mode {
        match UnixStream::connect(path) {
            Ok(s) => Ok(s),
            Err(e) => Err(Error::Communication(e)),
        }
    } else {
        unreachable!();
    }
}

pub type SocketListener = UnixListener;

pub fn get_socket_listener(session: &Session) -> Result<SocketListener> {
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
