use std::fs;
use std::io;
use std::ops::Deref;
use std::path::Path;

#[cfg(unix)]
use async_net::unix::{UnixListener, UnixStream};

#[cfg(windows)]
use super::socket_windows::{UnixListener, UnixStream};

#[derive(Debug)]
pub struct SocketListener(UnixListener);

impl SocketListener {
    pub async fn bind(path: &Path) -> io::Result<SocketListener> {
        let root_dir = path.parent().unwrap();
        if !root_dir.exists() {
            fs::create_dir_all(root_dir)?
        }
        Ok(SocketListener(UnixListener::bind(path)?))
    }
}

impl Deref for SocketListener {
    type Target = UnixListener;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type SocketStream = UnixStream;
