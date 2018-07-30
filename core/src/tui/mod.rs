mod not_unix;
mod unix;

#[cfg(not(unix))]
pub use self::not_unix::start;
#[cfg(unix)]
pub use self::unix::start;
