extern crate ced;
#[macro_use]
extern crate clap;
extern crate env_logger;
extern crate failure;
#[macro_use]
extern crate human_panic;

use clap::{App, Arg};
use failure::Error;

use ced::remote::{ensure_session, start_daemon, Session, StdioClient};
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
    let bin_path = std::env::args().next().unwrap();

    if matches.is_present("list") {
        for session_name in Session::list() {
            println!("{}", session_name)
        }
        Ok(())
    } else {
        let session = Session::from_name(
            matches
                .value_of("SESSION")
                .unwrap_or(&std::process::id().to_string()),
        );
        let filenames = match matches.values_of("FILE") {
            Some(args) => args.collect(),
            None => Vec::new(),
        };

        match value_t!(matches.value_of("MODE"), Mode).unwrap() {
            Mode::daemon => start_daemon(&bin_path, &session)
                .map(|pid| eprintln!("server started with pid {}", pid)),
            Mode::json => {
                ensure_session(&bin_path, &session)?;
                StdioClient::new(&session)?.run()
            }
            Mode::server => {
                eprintln!("starting server: {0} {0:?}", &session.mode);
                Server::new(session).run()
            }
            Mode::standalone => start_standalone(&filenames),
            Mode::term => {
                ensure_session(&bin_path, &session)?;
                start_tui(&session, &filenames)
            }
        }
    }
}
