#![allow(unknown_lints)]
#![warn(clippy)]

#[macro_use]
extern crate cfg_if;
#[macro_use]
extern crate crossbeam_channel;
extern crate env_logger;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate ignore;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate mio;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

cfg_if! {
    if #[cfg(unix)] {
        extern crate mio_uds;
        extern crate termion;
    } else if #[cfg(windows)] {
        extern crate mio_named_pipes;
        extern crate winapi;
    }
}

pub mod editor;
pub mod protocol;
pub mod remote;
pub mod server;
pub mod stackmap;
pub mod standalone;
pub mod tui;
