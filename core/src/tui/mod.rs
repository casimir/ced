mod term;

use failure::Error;

use remote::Session;

pub fn start(session: &Session, filenames: &[&str]) -> Result<(), Error> {
    if cfg!(unix) {
        self::term::start(session, filenames)
    } else {
        eprintln!("this mode is not supported on this platform");
        Ok(())
    }
}
