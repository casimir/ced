#![warn(clippy::all)]

pub use remote;

pub mod editor;
pub mod stackmap;
#[macro_use]
pub mod server;
pub mod standalone;

#[cfg(feature = "term")]
pub mod tui;
