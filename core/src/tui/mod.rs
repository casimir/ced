mod noterm;
mod term;

#[cfg(not(unix))]
pub use self::noterm::start;
#[cfg(unix)]
pub use self::term::start;
