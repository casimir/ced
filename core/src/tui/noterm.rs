#![cfg(not(unix))]

use failure::Error;

use remote::Session;

pub fn start(_session: &Session, _filenames: &[&str]) -> Result<(), Error> {
    eprintln!("this mode is not supported on this platform");
    Ok(())
}
