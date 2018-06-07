#[macro_use]
extern crate clap;
extern crate jsonrpc_lite;
extern crate mio;
extern crate mio_uds;
#[macro_use]
extern crate quick_error;
extern crate serde_json;

mod editor;
mod server;

use std::env;
use std::io::{self, Write};
use std::process::{Command, Stdio};

use clap::{App, Arg};
use jsonrpc_lite::JsonRpc;

use editor::Editor;
use server::{Server, ServerMode, SessionManager};

fn serialize_message(message: &JsonRpc) -> String {
    let json = serde_json::to_value(message).unwrap();
    serde_json::to_string(&json).unwrap()
}

fn start_standalone(filenames: Vec<&str>) {
    let mut editor = Editor::new(filenames);
    let (response, _) = editor.add_client(1).unwrap();
    writeln!(io::stdout(), "{}", serialize_message(&response));

    let mut buf = String::new();
    while let Ok(n) = io::stdin().read_line(&mut buf) {
        if n == 0 {
            break;
        }
        match editor.handle(1, &buf) {
            Ok((response, broadcast)) => {
                if let Some(msg) = broadcast {
                    println!("{}", serialize_message(&msg));
                }
                writeln!(io::stdout(), "{}", serialize_message(&response));
            }
            Err(e) => {
                writeln!(io::stderr(), "{}: {:?}", e, buf);
            }
        }
        buf.clear();
    }
    let _ = editor.remove_client(1);
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
            Arg::with_name("daemon")
                .short("d")
                .long("daemon")
                .help("Runs in headless mode"),
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
            Arg::with_name("standalone")
                .long("standalone")
                .conflicts_with("daemon")
                .conflicts_with("PORT")
                .conflicts_with("SESSION")
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
            start_standalone(filenames);
        } else if matches.is_present("daemon") {
            let args: Vec<String> = env::args()
                .filter(|a| a != "-d" && a != "--daemon")
                .collect();
            let prg = Command::new(&args[0])
                .args(&args[1..])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("failed to start daemon");
            println!("server started with pid: {}", prg.id());
        } else {
            let mode = if matches.is_present("PORT") {
                let addr = format!("127.0.0.1:{}", matches.value_of("PORT").unwrap());
                ServerMode::Tcp(addr)
            } else {
                let process_id = std::process::id().to_string();
                let session_name = matches.value_of("SESSION").unwrap_or(&process_id);
                ServerMode::UnixSocket(session_name.into())
            };
            let server = Server::new(mode);
            eprintln!("starting server: {:?}", server.mode);
            server.run(filenames).expect("failed to start server");
        }
    }
}
