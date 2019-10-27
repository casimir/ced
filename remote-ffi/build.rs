use std::env;

use ffigen::SourceBuilder;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    SourceBuilder::generate(&crate_dir)
        .expect("generate ffi")
        .write_to_file("src/ffi.rs")
        .expect("write ffi.rs");

    if env::var_os("CED_GEN_CHEADER").is_some() {
        cbindgen::generate(&crate_dir)
            .expect("generate bindings")
            .write_to_file("include/bindings.h");
    }
}
