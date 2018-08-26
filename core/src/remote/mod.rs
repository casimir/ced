mod client;
pub mod protocol;
mod session;
mod transport;

use failure::Error;
use tokio_core::reactor::Core;

pub use self::client::{Client, StdioClient};
pub use self::session::{ConnectionMode, Session};
pub use self::transport::{EventedStream, Listener, ServerConnection};

pub fn start_client(session: &Session) -> Result<(), Error> {
    let mut core = Core::new()?;
    let handle = core.handle();
    let client = StdioClient::new(&handle, session)?;
    core.run(client).expect("failed to start reactor");
    Ok(())
}
