#[macro_use]
extern crate clap;
extern crate env_logger;
extern crate jsonrpc_lite;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate mio;
#[macro_use]
extern crate quick_error;
extern crate regex;
extern crate serde_json;

#[cfg(unix)]
extern crate mio_uds;

#[cfg(windows)]
extern crate mio_named_pipes;
#[cfg(windows)]
extern crate winapi;

mod editor;
mod remote;
mod server;
mod stackmap;
mod standalone;

use std::env;
use std::io;
use std::process::{self, Command, Stdio};
use std::thread;
use std::time::Duration;

use clap::{App, Arg};

use remote::{connect, ConnectionMode, Session};
use server::Server;
use standalone::start_standalone;

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

fn start_daemon(session: &Session, filenames: &[&str]) {
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
        .spawn()
        .expect("failed to start daemon");
    eprintln!("server started with pid {}", prg.id());
    info!("server command: {:?}", args)
}

// TODO return Result
fn main() {
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
                .default_value("json")
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
            Mode::daemon => start_daemon(&session, &filenames),
            Mode::json => {
                if let ConnectionMode::Socket(path) = &session.mode {
                    if !path.exists() {
                        start_daemon(&session, &filenames);
                        // ensures the daemon process got time to create the socket
                        thread::sleep(Duration::from_millis(100));
                    }
                }
                let stdin = io::stdin();
                connect(&session, &mut stdin.lock(), Box::new(io::stdout()))
                    .expect("failed to connect");
            }
            Mode::server => {
                eprintln!("starting server: {0} {0:?}", &session.mode);
                let server = Server::new(session);
                server.run(&filenames).expect("failed to start server");
            }
            Mode::standalone => {
                let stdin = io::stdin();
                start_standalone(
                    &mut stdin.lock(),
                    &mut io::stdout(),
                    &mut io::stderr(),
                    &filenames,
                );
            }
            Mode::term => unimplemented!(),
        }
    }
}
