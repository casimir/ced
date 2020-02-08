use std::env::current_dir;
use std::fs::{self, File};
use std::io::Read;
use std::ops::Index;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::editor::view::Focus;
use crate::editor::PieceTable;

fn find_uniq_name(path: &PathBuf, acc: &str, path_set: &[PathBuf]) -> String {
    let head = path.parent().unwrap();
    let tail = path.file_name().unwrap();
    let matches = path_set
        .iter()
        .filter(|x| x.file_name().map_or(false, |x| x.to_str().unwrap() == tail))
        .count();
    let mut new_acc = tail.to_str().unwrap().to_owned();
    if !acc.is_empty() {
        new_acc.push('/');
        new_acc.push_str(acc);
    }
    if matches > 1 {
        let mut new_path_set: Vec<PathBuf> = Vec::new();
        for pb in path_set {
            let mut new_pb = PathBuf::from(pb);
            if new_pb.pop() {
                new_path_set.push(new_pb);
            }
        }
        find_uniq_name(&head.to_owned(), &new_acc, &new_path_set)
    } else {
        new_acc
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BufferSource {
    Scratch(String),
    File(PathBuf),
}

fn find_shortest_name(sources: &[BufferSource], idx: usize) -> String {
    let source = sources.index(idx);
    match *source {
        BufferSource::Scratch(ref name) => name.clone(),
        BufferSource::File(ref path) => {
            let mut path_set: Vec<PathBuf> = Vec::new();
            for s in sources {
                if let BufferSource::File(ref path) = *s {
                    path_set.push(path.clone());
                }
            }
            find_uniq_name(path, "", &path_set)
        }
    }
}

pub struct Buffer {
    pub source: BufferSource,
    pub content: PieceTable,
    last_sync: Option<SystemTime>,
    modified: bool,
}

impl Buffer {
    pub fn new_scratch(name: String) -> Buffer {
        Buffer {
            source: BufferSource::Scratch(name),
            content: PieceTable::new(),
            last_sync: None,
            modified: false,
        }
    }

    pub fn new_file(filename: &PathBuf) -> Buffer {
        let absolute_path = if filename.is_absolute() {
            filename.clone()
        } else {
            let mut full_path = current_dir().unwrap();
            full_path.push(filename.clone());
            full_path.as_path().canonicalize().unwrap()
        };

        let mut file = File::open(&absolute_path).unwrap();
        let mut file_content = String::new();
        file.read_to_string(&mut file_content).expect("read file");
        let last_sync = Some(SystemTime::now());

        Buffer {
            source: BufferSource::File(absolute_path),
            content: PieceTable::with_text(file_content),
            last_sync,
            modified: false,
        }
    }

    pub fn line_count(&self) -> usize {
        self.content.line_count()
    }

    pub fn lines(&self, focus: Focus) -> Vec<String> {
        match focus {
            Focus::Range(range) => self.content.lines()[range].to_vec(),
            Focus::Whole => self.content.lines(),
        }
    }

    pub fn shortest_name(&self, sources: &[BufferSource]) -> String {
        let idx = sources.iter().position(|x| *x == self.source).unwrap();
        find_shortest_name(sources, idx)
    }

    fn is_synced(&self) -> bool {
        if let BufferSource::File(ref path) = self.source {
            let dt_sync = match self.last_sync {
                Some(dt) => dt,
                None => return false,
            };
            let meta = match fs::metadata(path) {
                Ok(data) => data,
                Err(_) => return false,
            };
            if !meta.is_file() {
                return false;
            }
            match meta.modified() {
                Ok(dt_modified) => return dt_modified < dt_sync,
                Err(_) => return false,
            }
        }
        true
    }

    pub fn load_from_disk(&mut self) -> bool {
        match self.source {
            BufferSource::Scratch(_) => true,
            BufferSource::File(ref path) => {
                if !self.is_synced() {
                    let mut file = File::open(&path).unwrap();
                    let mut content = String::new();
                    file.read_to_string(&mut content).expect("read file");
                    self.content.apply_diff(&content);
                    self.last_sync = Some(SystemTime::now());
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn append(&mut self, text: String) {
        self.content.append(text);
        self.modified = true;
    }
}

impl ToString for Buffer {
    fn to_string(&self) -> String {
        let mut content = self.content.text();
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_name() {
        let s_debug = BufferSource::Scratch("*buffer*".into());
        let f_some = BufferSource::File(PathBuf::from("/some/file.ext"));
        let f_where_1 = BufferSource::File(PathBuf::from("/some/where/file.where"));
        let f_where_2 = BufferSource::File(PathBuf::from("/any/where/file.where"));
        let f_where_3 = BufferSource::File(PathBuf::from("/any/where/here/file.where"));

        let sources = [s_debug, f_some, f_where_1, f_where_2, f_where_3];

        assert_eq!(find_shortest_name(&sources, 0), "*buffer*");
        assert_eq!(find_shortest_name(&sources, 1), "file.ext");
        assert_eq!(find_shortest_name(&sources, 2), "some/where/file.where");
        assert_eq!(find_shortest_name(&sources, 3), "any/where/file.where");
        assert_eq!(find_shortest_name(&sources, 4), "here/file.where");
    }

    #[test]
    fn open_file() {
        let filename = PathBuf::from("Cargo.toml");
        let mut file = File::open(&filename).expect("opening file");
        let mut content = String::new();
        file.read_to_string(&mut content).expect("reading file");
        let lines: Vec<String> = content.lines().map(ToOwned::to_owned).collect();
        let mut full_path = current_dir().unwrap();
        full_path.push(filename.clone());

        let buffer = Buffer::new_file(&filename);
        let source = BufferSource::File(full_path.as_path().canonicalize().unwrap());

        assert_eq!(buffer.source, source);
        assert!(buffer.last_sync.is_some());
        assert_eq!(buffer.lines(Focus::Whole), lines);
    }
}
