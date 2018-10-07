extern crate ced;
#[macro_use]
extern crate clap;
extern crate env_logger;
extern crate failure;
#[macro_use]
extern crate human_panic;
#[macro_use]
extern crate log;

use std::env;
use std::process::{self, Command, Stdio};
use std::thread;
use std::time::Duration;

use clap::{App, Arg};
use failure::Error;

use ced::remote::{ConnectionMode, Session, StdioClient};
use ced::server::Server;
use ced::standalone::start_standalone;
use ced::tui::start as start_tui;

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

fn start_daemon(session: &Session) -> Result<u32, Error> {
    let args = vec![
        env::args().next().unwrap(),
        "--mode=server".to_string(),
        format!("--session={}", session.mode),
    ];
    let prg = Command::new(&args[0])
        .args(&args[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    info!("server command: {:?}", args);
    Ok(prg.id())
}

fn ensure_session(session: &Session) -> Result<(), Error> {
    if let ConnectionMode::Socket(path) = &session.mode {
        if !path.exists() {
            start_daemon(&session)?;
            // ensures the daemon process got time to create the socket
            thread::sleep(Duration::from_millis(150));
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
        ).arg(
            Arg::with_name("SESSION")
                .short("s")
                .long("session")
                .takes_value(true)
                .help("Sets session name"),
        ).arg(
            Arg::with_name("MODE")
                .short("m")
                .long("mode")
                .possible_values(&Mode::variants())
                .default_value(Mode::default_value())
                .help("Mode to use"),
        ).arg(
            Arg::with_name("FILE")
                .multiple(true)
                .help("A list of files to open"),
        ).get_matches();

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
            Mode::daemon => {
                start_daemon(&session).map(|pid| eprintln!("server started with pid {}", pid))
            }
            Mode::json => {
                ensure_session(&session)?;
                StdioClient::new(&session)?.run()
            }
            Mode::server => {
                eprintln!("starting server: {0} {0:?}", &session.mode);
                Server::new(session).run()
            }
            Mode::standalone => start_standalone(&filenames),
            Mode::term => {
                ensure_session(&session)?;
                start_tui(&session, &filenames)
            }
        }
    }
}
