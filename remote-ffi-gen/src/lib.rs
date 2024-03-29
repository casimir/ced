use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

fn to_snake_case(sym: &str) -> String {
    let mut snake = String::new();
    for (i, ch) in sym.char_indices() {
        if i > 0 && ch.is_uppercase() {
            snake.push('_');
        }
        snake.push(ch.to_ascii_lowercase());
    }
    snake
}

fn default_header() -> String {
    "// This file is automatically generated when building.".to_string()
}

fn default_module() -> String {
    String::from("crate")
}

#[derive(Clone, Default, Deserialize)]
struct Functions {
    #[serde(default)]
    iterator: bool,
}

#[derive(Clone, Deserialize)]
struct Struct {
    symbol: String,
    #[serde(default = "default_module")]
    module: String,
    #[serde(default)]
    functions: Functions,
}

impl Struct {
    fn bare_type(&self) -> String {
        if self.symbol.ends_with("Item") {
            self.symbol[..self.symbol.len() - 4].to_owned()
        } else if self.symbol.ends_with("Iterator") {
            self.symbol[..self.symbol.len() - 8].to_owned()
        } else {
            self.symbol.to_owned()
        }
    }

    fn iterator_struct(&self) -> Struct {
        Struct {
            symbol: self.bare_type() + "Iterator",
            module: self.module.clone(),
            functions: Functions::default(),
        }
    }
}

#[derive(Deserialize)]
struct Config {
    #[serde(default = "default_header")]
    header: String,
    #[serde(default)]
    prefix: String,
    structure: Vec<Struct>,
}

pub struct SourceBuilder {
    working_dir: PathBuf,
    config: Config,
    uses: Vec<String>,
    decls: Vec<String>,
}

impl SourceBuilder {
    pub fn new<P: AsRef<Path>>(working_dir: P) -> io::Result<SourceBuilder> {
        let path = working_dir.as_ref().join("ffigen.toml");
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut toml = String::new();
        reader.read_to_string(&mut toml)?;
        let config: Config = toml::from_str(&toml)?;
        Ok(SourceBuilder {
            working_dir: working_dir.as_ref().to_owned(),
            config,
            uses: Vec::new(),
            decls: Vec::new(),
        })
    }

    fn register_use(&mut self, st: &Struct) {
        self.uses
            .push(format!("use {0}::{1};", st.module, st.symbol));
    }

    fn register_decl(&mut self, source: String) {
        self.decls.push(source);
    }

    fn register_fn_destroy(&mut self, st: &Struct) {
        self.register_use(st);

        let mut fn_name = to_snake_case(&st.symbol) + "_destroy";
        if !fn_name.starts_with(&self.config.prefix) {
            fn_name = format!("{}{}", &self.config.prefix, fn_name);
        }
        self.register_decl(
            stringify!(
                #[no_mangle]
                pub unsafe extern "C" fn _FUNCTION_(p: *mut _SYM_) {
                    if !p.is_null() {
                        drop(Box::from_raw(p))
                    }
                }
            )
            .replace("_FUNCTION_", &fn_name)
            .replace("_SYM_", &st.symbol)
                + "\n",
        );
    }

    fn register_fn_next_item(&mut self, st: &Struct, item_st: &Struct) {
        let mut fn_name = to_snake_case(&st.bare_type()) + "_next_item";
        if !fn_name.starts_with(&self.config.prefix) {
            fn_name = format!("{}{}", &self.config.prefix, fn_name);
        }
        self.register_decl(
            stringify!(
                #[no_mangle]
                pub unsafe extern "C" fn _FUNCTION_(p: *mut _SYM_) -> *mut _ITEM_ {
                    let iterator = &mut *p;
                    match iterator.next() {
                        Some(item) => Box::into_raw(Box::new(item)),
                        None => ptr::null_mut(),
                    }
                }
            )
            .replace("_FUNCTION_", &fn_name)
            .replace("_SYM_", &st.symbol)
            .replace("_ITEM_", &item_st.symbol)
                + "\n",
        );
    }

    fn register_fns_iterator(&mut self, st: &Struct) {
        self.register_use(st);
        self.uses.push("use std::ptr;".to_string());

        let it = &st.iterator_struct();
        self.register_fn_destroy(it);
        self.register_fn_next_item(it, st);
    }

    pub fn generate<P: AsRef<Path>>(working_dir: P) -> io::Result<SourceBuilder> {
        let mut builder = SourceBuilder::new(working_dir)?;
        for st in &builder.config.structure.clone() {
            builder.register_fn_destroy(st);
            if st.functions.iterator {
                builder.register_fns_iterator(st);
            }
        }
        builder.uses.sort();
        builder.uses.dedup();
        Ok(builder)
    }
    pub fn source(&self) -> String {
        vec![
            self.config.header.clone(),
            String::new(),
            self.uses.join("\n"),
            String::new(),
            self.decls.join("\n"),
        ]
        .join("\n")
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let dest = self.working_dir.join(path.as_ref());
        let mut f = File::create(&dest)?;
        f.write_all(self.source().as_bytes())?;
        let _ = Command::new("cargo")
            .arg("fmt")
            .arg("--")
            .arg(dest.display().to_string())
            .spawn();
        Ok(())
    }
}
