#![allow(unknown_lints)]
#![warn(clippy)]

#[macro_use]
extern crate crossbeam_channel;
extern crate failure;
#[macro_use]
extern crate failure_derive;
pub extern crate ced_remote as remote;
extern crate ignore;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate mio;
extern crate regex;
#[cfg(unix)]
extern crate termion;

pub mod editor;
#[macro_use]
pub mod server;
pub mod stackmap;
pub mod standalone;
pub mod tui;
