use std::env::current_dir;
use std::fs::{self, File};
use std::io::Read;
use std::ops::Index;
use std::path::PathBuf;
use std::time::SystemTime;

fn find_uniq_name(path: &PathBuf, acc: &str, path_set: &Vec<PathBuf>) -> String {
    let head = path.parent().unwrap();
    let tail = path.file_name().unwrap();
    let matches = path_set
        .iter()
        .filter(|x| {
            x.file_name().map_or(false, |x| x.to_str().unwrap() == tail)
        })
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

pub fn find_shortest_name(sources: &Vec<&BufferSource>, idx: usize) -> String {
    let source = sources.index(idx);
    match **source {
        BufferSource::Scratch(ref name) => name.clone(),
        BufferSource::File(ref path) => {
            let mut path_set: Vec<PathBuf> = Vec::new();
            for s in sources {
                if let BufferSource::File(ref path) = **s {
                    path_set.push(path.clone());
                }
            }
            find_uniq_name(path, "", &path_set)
        }
    }
}

#[derive(Clone)]
pub struct Buffer {
    pub source: BufferSource,
    lines: Vec<String>,
    last_sync: Option<SystemTime>,
}

impl Buffer {
    pub fn new_scratch(name: String) -> Buffer {
        Buffer {
            source: BufferSource::Scratch(name),
            lines: Vec::new(),
            last_sync: None,
        }
    }

    pub fn new_file(filename: PathBuf) -> Buffer {
        let mut full_path = current_dir().unwrap();
        full_path.push(filename.clone());
        let absolute_path = full_path.as_path().canonicalize().unwrap();

        let mut buffer = Buffer {
            source: BufferSource::File(absolute_path),
            lines: Vec::new(),
            last_sync: None,
        };
        buffer.load_from_disk(true);
        buffer
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

    pub fn load_from_disk(&mut self, force: bool) {
        if force || !self.is_synced() {
            if let BufferSource::File(ref path) = self.source {
                let mut file = File::open(&path).unwrap();
                let mut content = String::new();
                file.read_to_string(&mut content);
                self.lines = content.lines().map(ToOwned::to_owned).collect();
                self.last_sync = Some(SystemTime::now());
            }
        }
    }

    pub fn append(&mut self, content: &str) {
        self.lines.push(content.to_owned());
    }
}

impl ToString for Buffer {
    fn to_string(&self) -> String {
        let mut content = self.lines.join("\n").to_owned();
        if !content.is_empty() && content.chars().last() != Some('\n') {
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
        let s_debug = BufferSource::Scratch("*debug*".into());
        let f_some = BufferSource::File(PathBuf::from("/some/file.ext"));
        let f_where_1 = BufferSource::File(PathBuf::from("/some/where/file.where"));
        let f_where_2 = BufferSource::File(PathBuf::from("/any/where/file.where"));
        let f_where_3 = BufferSource::File(PathBuf::from("/any/where/here/file.where"));

        let sources = vec![&s_debug, &f_some, &f_where_1, &f_where_2, &f_where_3];

        assert!(find_shortest_name(&sources, 0) == "*debug*");
        assert!(find_shortest_name(&sources, 1) == "file.ext");
        assert!(find_shortest_name(&sources, 2) == "some/where/file.where");
        assert!(find_shortest_name(&sources, 3) == "any/where/file.where");
        assert!(find_shortest_name(&sources, 4) == "here/file.where");
    }
}
