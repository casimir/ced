#![warn(clippy::all)]

#[macro_use]
extern crate crossbeam_channel;
extern crate failure;
#[macro_use]
extern crate failure_derive;
pub extern crate ced_remote as remote;
extern crate ignore;
#[macro_use]
extern crate log;
extern crate mio;
extern crate regex;

pub mod datastruct;
pub mod editor;
#[macro_use]
pub mod server;
pub mod standalone;

#[cfg(all(feature = "term", unix))]
extern crate termion;

#[cfg(all(feature = "term", unix))]
pub mod tui;
