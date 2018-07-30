#![cfg(not(unix))]

use remote::Session;

pub fn start(session: &Session, filenames: &[&str]) {
    eprintln!("this mode is not supported on this platform");
}
