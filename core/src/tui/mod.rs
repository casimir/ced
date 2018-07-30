#[cfg(not(unix))]
use remote::{Result, Session};

mod term;

#[cfg(unix)]
pub use self::term::start;

#[cfg(not(unix))]
pub fn start(_session: &Session, _filenames: &[&str]) -> Result<()> {
    eprintln!("this mode is not supported on this platform");
    Ok(())
}
