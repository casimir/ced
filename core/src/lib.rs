#![warn(clippy::all)]
#![recursion_limit = "256"]

pub use remote;

pub mod editor;
pub mod stackmap;
#[macro_use]
pub mod server;
pub mod script;
pub mod standalone;

#[cfg(feature = "term")]
pub mod tui;
