#[macro_use]
extern crate clap;
extern crate jsonrpc_lite;
extern crate mio;
extern crate mio_uds;
#[macro_use]
extern crate quick_error;
extern crate serde_json;

mod editor;
mod remote;
mod server;
mod standalone;

use std::env;
use std::io;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use clap::{App, Arg, ArgMatches};

use remote::connect;
use server::{Server, ServerMode, SessionManager};
use standalone::start_standalone;

fn get_server_mode(matches: &ArgMatches) -> Option<ServerMode> {
    if matches.is_present("PORT") {
        let addr = format!("127.0.0.1:{}", matches.value_of("PORT").unwrap());
        Some(ServerMode::Tcp(addr))
    } else if matches.is_present("SESSION") {
        let session_name = matches.value_of("SESSION").unwrap();
        Some(ServerMode::UnixSocket(session_name.into()))
    } else {
        None
    }
}

fn get_server_mode_or_default(matches: &ArgMatches) -> ServerMode {
    let process_id = std::process::id().to_string();
    get_server_mode(matches).unwrap_or(ServerMode::UnixSocket(process_id))
}

fn start_daemon_with_args(args: Vec<String>) {
    let prg = Command::new(&args[0])
        .args(&args[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start daemon");
    eprintln!("server started with pid: {}", prg.id());
}

fn start_daemon() {
    let args = env::args()
        .map(|a| {
            // TODO replace hidden argument by double fork
            if a == "-d" || a == "--daemon" {
                "--server".into()
            } else {
                a
            }
        })
        .filter(|a| a != "-d" && a != "--daemon")
        .collect();
    start_daemon_with_args(args);
}

fn main() {
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
            Arg::with_name("PORT")
                .short("p")
                .long("port")
                .takes_value(true)
                .conflicts_with("SESSION")
                .help("Sets server port (implies TCP mode)"),
        )
        .arg(
            Arg::with_name("daemon")
                .short("d")
                .long("daemon")
                .help("Starts a server in headless mode"),
        )
        .arg(Arg::with_name("server").long("server").hidden(true))
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
        let sm = SessionManager::new();
        for session_name in &sm.list() {
            println!("{}", session_name)
        }
    } else {
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
            let mode = get_server_mode_or_default(&matches);
            let server = Server::new(mode);
            eprintln!("starting server: {:?}", server.mode);
            server.run(filenames).expect("failed to start server");
        } else {
            let mode = get_server_mode_or_default(&matches);
            match &mode {
                #[cfg(unix)]
                ServerMode::UnixSocket(name) => {
                    if !SessionManager::new().exists(&name) {
                        let mut args = vec![
                            env::args().next().unwrap(),
                            "--daemon".to_string(),
                            "--session".to_string(),
                            name.clone(),
                        ];
                        for filename in &filenames {
                            args.push(filename.to_string());
                        }
                        start_daemon_with_args(args);
                        // ensures the daemon process got time to create the socket
                        thread::sleep(Duration::from_millis(100));
                    }
                }
                _ => {}
            }
            let stdin = io::stdin();
            connect(&mut stdin.lock(), Box::new(io::stdout()), &mode);
        }
    }
}
