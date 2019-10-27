#![warn(clippy::all)]

#[macro_use]
extern crate crossbeam_channel;
pub extern crate ced_remote as remote;
extern crate ignore;
#[macro_use]
extern crate log;
extern crate mio;
extern crate regex;

pub mod editor;
pub mod stackmap;
#[macro_use]
pub mod server;
pub mod standalone;

#[cfg(all(feature = "term", unix))]
extern crate termion;

#[cfg(all(feature = "term", unix))]
pub mod tui;
