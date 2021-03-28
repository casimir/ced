use std::io;

use ced::clients::{start_standalone, StdioClient};
use ced::remote::{ensure_session, start_daemon, Session};
use ced::script::exec_scripts;
use ced::server::Server;
use clap::{arg_enum, crate_authors, crate_description, crate_version, value_t, App, Arg};

#[cfg(feature = "term")]
arg_enum! {
    #[allow(non_camel_case_types)]
    #[derive(Debug)]
    pub enum Mode {
        daemon,
        json,
        script,
        server,
        standalone,
        term,
    }
}
#[cfg(not(feature = "term"))]
arg_enum! {
    #[allow(non_camel_case_types)]
    #[derive(Debug)]
    pub enum Mode {
        daemon,
        json,
        script,
        server,
        standalone,
    }
}

impl Mode {
    fn default_value() -> &'static str {
        if cfg!(feature = "term") {
            "term"
        } else {
            "json"
        }
    }
}

fn main() -> io::Result<()> {
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
        let session = match matches.value_of("SESSION") {
            Some(name) => Session::from_name(name),
            None => Session::from_pid(),
        };
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
                StdioClient::new(session).run()
            }
            Mode::script => exec_scripts(&filenames),
            Mode::server => {
                eprintln!("starting server: {0} {0:?}", &session.mode);
                Server::new(session).run()
            }
            Mode::standalone => {
                start_standalone(&filenames);
                Ok(())
            }
            #[cfg(feature = "term")]
            Mode::term => {
                use ced::tui::Term;
                ensure_session(&session)?;
                Term::new(session, &filenames).start();
                Ok(())
            }
        }
    }
}
