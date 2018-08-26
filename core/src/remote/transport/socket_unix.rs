#![cfg(unix)]

use std::fs;

use failure::Error;
use mio_uds::UnixListener;

use remote::{ConnectionMode, Session};

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
