#[macro_use]
extern crate clap;
extern crate jsonrpc_lite;
extern crate serde_json;

mod editor;

use std::io::{self, BufRead};

use clap::{App, Arg};

use editor::Editor;

fn main() {
    let matches = App::new("ced")
        .about(crate_description!())
        .version(crate_version!())
        .author(crate_authors!())
        .arg(
            Arg::with_name("FILE")
                .multiple(true)
                .help("a list of file to open"),
        )
        .get_matches();

    let files = match matches.values_of("FILE") {
        Some(args) => args.collect(),
        None => Vec::new(),
    };
    let mut editor = Editor::new(files);

    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut buf = String::new();
    editor.connect();
    while handle.read_line(&mut buf).is_ok() {
        if buf.is_empty() {
            break;
        }
        editor.handle(&buf);
        buf.clear();
    }
}
