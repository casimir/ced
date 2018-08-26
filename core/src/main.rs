#![allow(unknown_lints)]
#![warn(clippy)]

#[macro_use]
extern crate cfg_if;
#[macro_use]
extern crate clap;
extern crate env_logger;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate futures;
extern crate jsonrpc_lite;
#[macro_use]
extern crate human_panic;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate mio;
extern crate regex;
extern crate serde_json;
extern crate tokio;
extern crate tokio_core;

cfg_if! {
    if #[cfg(unix)] {
        extern crate ignore;
        extern crate mio_uds;
        extern crate termion;
    } else if #[cfg(windows)] {
        extern crate mio_named_pipes;
        extern crate tokio_named_pipes;
        extern crate winapi;
    }
}

mod editor;
mod remote;
mod server;
mod stackmap;
mod standalone;
mod tui;

use std::env;
use std::io;
use std::process::{self, Command, Stdio};
use std::thread;
use std::time::Duration;

use clap::{App, Arg};
use failure::Error;

use remote::{start_client, ConnectionMode, Session};
use server::Server;
use standalone::start_standalone;
use tui::start as start_tui;

arg_enum!{
    #[allow(non_camel_case_types)]
    #[derive(Debug)]
    pub enum Mode {
        daemon,
        json,
        server,
        standalone,
        term,
    }
}

impl Mode {
    fn default_value() -> &'static str {
        if cfg!(unix) {
            "term"
        } else {
            "json"
        }
    }
}

fn start_daemon(session: &Session, filenames: &[&str], quiet: bool) -> Result<(), Error> {
    let mut args = vec![
        env::args().next().unwrap(),
        "--mode=server".to_string(),
        format!("--session={}", session.mode),
    ];
    for filename in filenames {
        args.push(filename.to_string());
    }
    let prg = Command::new(&args[0])
        .args(&args[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    if !quiet {
        eprintln!("server started with pid {}", prg.id());
    }
    info!("server command: {:?}", args);
    Ok(())
}

fn ensure_session(session: &Session, filenames: &[&str]) -> Result<(), Error> {
    if let ConnectionMode::Socket(path) = &session.mode {
        if !path.exists() {
            start_daemon(&session, &filenames, true)?;
            // ensures the daemon process got time to create the socket
            thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}

fn main() -> Result<(), Error> {
    setup_panic!();
    env_logger::init();

    let matches = App::new("ced")
        .about(crate_description!())
        .version(crate_version!())
        .author(crate_authors!())
        .arg(
            Arg::with_name("list")
                .short("l")
                .long("list")
                .help("Lists running sessions"),
        )
        .arg(
            Arg::with_name("SESSION")
                .short("s")
                .long("session")
                .takes_value(true)
                .help("Sets session name"),
        )
        .arg(
            Arg::with_name("MODE")
                .short("m")
                .long("mode")
                .possible_values(&Mode::variants())
                .default_value(Mode::default_value())
                .help("Mode to use"),
        )
        .arg(
            Arg::with_name("FILE")
                .multiple(true)
                .help("A list of files to open"),
        )
        .get_matches();

    if matches.is_present("list") {
        for session_name in Session::list() {
            println!("{}", session_name)
        }
        Ok(())
    } else {
        let session = Session::from_name(
            matches
                .value_of("SESSION")
                .unwrap_or(&process::id().to_string()),
        );
        let filenames = match matches.values_of("FILE") {
            Some(args) => args.collect(),
            None => Vec::new(),
        };

        match value_t!(matches.value_of("MODE"), Mode).unwrap() {
            Mode::daemon => start_daemon(&session, &filenames, false),
            Mode::json => {
                ensure_session(&session, &filenames)?;
                start_client(&session)
            }
            Mode::server => {
                eprintln!("starting server: {0} {0:?}", &session.mode);
                let server = Server::new(session);
                server.run(&filenames)
            }
            Mode::standalone => {
                let stdin = io::stdin();
                start_standalone(
                    &mut stdin.lock(),
                    &mut io::stdout(),
                    &mut io::stderr(),
                    &filenames,
                )?;
                Ok(())
            }
            Mode::term => {
                ensure_session(&session, &filenames)?;
                start_tui(&session, &filenames)
            }
        }
    }
}
