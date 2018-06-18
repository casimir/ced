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

fn start_daemon_with_args(args: Vec<String>) {
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

fn start_daemon() {
    let args = env::args()
        .map(|a| {
            if a == "-d" || a == "--daemon" {
                "--server".into()
            } else {
                a
            }
        })
        .collect();
    start_daemon_with_args(args);
}

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
            Arg::with_name("daemon")
                .short("d")
                .long("daemon")
                .help("Starts a server in headless mode"),
        )
        .arg(Arg::with_name("server").long("server").hidden(true)) // TODO replace hidden argument by double fork
        .arg(
            Arg::with_name("standalone")
                .long("standalone")
                .conflicts_with("daemon")
                .help("Uses standalone mode (1 client for 1 server in-process)"),
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

        if matches.is_present("standalone") {
            let stdin = io::stdin();
            start_standalone(
                &mut stdin.lock(),
                &mut io::stdout(),
                &mut io::stderr(),
                filenames,
            );
        } else if matches.is_present("daemon") {
            start_daemon();
        } else if matches.is_present("server") {
            eprintln!("starting server: {0} {0:?}", &session.mode);
            let server = Server::new(session);
            server.run(filenames).expect("failed to start server");
        } else {
            if let ConnectionMode::Socket(path) = &session.mode {
                if !path.exists() {
                    let mut args = vec![
                        env::args().next().unwrap(),
                        "--daemon".to_string(),
                        "--session".to_string(),
                        format!("{}", session.mode),
                    ];
                    for filename in &filenames {
                        args.push(filename.to_string());
                    }
                    start_daemon_with_args(args);
                    // ensures the daemon process got time to create the socket
                    thread::sleep(Duration::from_millis(100));
                }
            }
            let stdin = io::stdin();
            connect(&session, &mut stdin.lock(), Box::new(io::stdout()))
                .expect("failed to connect");
        }
    }
}
