mod server_listener;
mod server_stream;
mod socket;

use std::io;

use mio::Evented;

pub use self::server_listener::ServerListener;
pub use self::server_stream::ServerStream;

pub trait Stream: io::Read + io::Write + Send {}

impl<T> Stream for T where T: io::Read + io::Write + Send {}

pub trait EventedStream: Stream + Evented {}

impl<T> EventedStream for T where T: Stream + Evented {}
