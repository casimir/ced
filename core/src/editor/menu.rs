use std::cmp::Ordering;
use std::fmt;
use std::ops::Deref;

use ignore::Walk;
use regex::{CaptureLocations, Regex};

#[derive(Debug)]
pub struct Candidate {
    pub text: String,
    score: Option<f32>,
    locations: CaptureLocations,
}

impl Candidate {
    fn new(re: &Regex, text: &str) -> Candidate {
        let mut locations = re.capture_locations();
        let score = re
            .captures_read(&mut locations, &text)
            .map(|_| Candidate::compute_score(&locations));
        Candidate {
            text: text.to_owned(),
            score,
            locations,
        }
    }

    fn compute_score(locations: &CaptureLocations) -> f32 {
        if locations.len() > 0 {
            let (start, _) = locations.get(0).unwrap();
            let (_, end) = locations.get(locations.len() - 1).unwrap();
            100.0 / (1 + end - start) as f32
        } else {
            0.0
        }
    }

    pub fn is_match(&self) -> bool {
        self.score.is_some()
    }

    pub fn decorate(&self, decorator: &Fn(&str) -> String) -> String {
        let mut decorated = self.text.clone();
        let mut offset = 0;
        for i in 1..self.locations.len() {
            if let Some((start, end)) = self.locations.get(i) {
                let decorated_part = decorator(&self.text[start..end]);
                let (ostart, oend) = (start + offset, end + offset);
                decorated.replace_range(ostart..oend, &decorated_part);
                offset += decorated_part.len() - (end - start);
            }
        }
        decorated
    }
}

impl fmt::Display for Candidate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.2} {}", self.score.unwrap_or(-1.0), self.text)
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Candidate) -> Ordering {
        if self == other {
            other.text.cmp(&self.text)
        } else if self.score > other.score {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Candidate) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Candidate) -> bool {
        self.score == other.score
    }
}

impl Eq for Candidate {}

#[derive(Default)]
pub struct Candidates(Vec<Candidate>);

impl Deref for Candidates {
    type Target = Vec<Candidate>;

    fn deref(&self) -> &Vec<Candidate> {
        &self.0
    }
}

pub struct MenuFilter {
    pub search: String,
}

impl MenuFilter {
    pub fn new(search: &str) -> MenuFilter {
        MenuFilter {
            search: search.to_string(),
        }
    }

    fn build_regex(&self) -> Regex {
        let elements: Vec<String> = self
            .search
            .split_whitespace()
            .map(|e| format!("({})", e))
            .collect();
        let raw_re = format!("(?i){}", elements.join(".*"));
        Regex::new(&raw_re).unwrap()
    }

    pub fn filter(&self, items: &[String]) -> Candidates {
        let re = self.build_regex();
        let mut candidates: Vec<Candidate> = items.iter().map(|i| Candidate::new(&re, i)).collect();
        candidates.sort_by(|a, b| b.cmp(a));
        Candidates(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_path() {
        let items = vec![
            "/abs/path/here".into(),
            "/tmp".into(),
            "/project/module/file.ext".into(),
            "/project/modula/file.ext".into(),
            "/project/submodule/file.ext".into(),
            "/project/file.ext".into(),
            "/project/file".into(),
            "/project/amodule/file.ext".into(),
            "/proj/file/ext/other.ext".into(),
        ];
        let expected = vec![
            "/project/file.ext",           // shortest match
            "/proj/file/ext/other.ext",    // longer match
            "/project/modula/file.ext",    // longer match (a)
            "/project/module/file.ext",    // longer match (e)
            "/project/amodule/file.ext",   // longer match
            "/project/submodule/file.ext", // longer match
            "/abs/path/here",              // no match (a)
            "/project/file",               // no match (p)
            "/tmp",                        // no match (t)
        ];
        let f = MenuFilter::new("proj fil ext");
        let candidates = &f.filter(&items);
        let res: Vec<String> = candidates.iter().map(|ref c| c.text.clone()).collect();
        assert_eq!(res, expected);
    }

    #[test]
    fn test_multiple_searches() {
        let f = MenuFilter::new("proj fil ext");

        let items1 = vec![
            "/abs/path/here".into(),
            "/project/module/file.ext".into(),
            "/project/submodule/file.ext".into(),
            "/project/file".into(),
            "/proj/file/ext/other.ext".into(),
        ];
        let expected1 = vec![
            "/proj/file/ext/other.ext",
            "/project/module/file.ext",
            "/project/submodule/file.ext",
            "/abs/path/here",
            "/project/file",
        ];
        let candidates1 = &f.filter(&items1);
        let res1: Vec<String> = candidates1.iter().map(|ref c| c.text.clone()).collect();

        let items2 = vec![
            "/tmp".into(),
            "/project/modula/file.ext".into(),
            "/project/file.ext".into(),
            "/project/amodule/file.ext".into(),
        ];
        let expected2 = vec![
            "/project/file.ext",
            "/project/modula/file.ext",
            "/project/amodule/file.ext",
            "/tmp",
        ];
        let candidates2 = &f.filter(&items2);
        let res2: Vec<String> = candidates2.iter().map(|ref c| c.text.clone()).collect();

        assert_eq!(res1, expected1);
        assert_eq!(res2, expected2);
    }

    #[test]
    fn test_decorate() {
        let items = vec![
            "/project/src/file.ext".into(),
            "project/no/match/file.ext".into(),
        ];
        let upper_fn = |cap: &str| cap.chars().flat_map(char::to_uppercase).collect();
        let f = MenuFilter::new("proj src ext");
        let candidates = &f.filter(&items);
        assert_eq!(candidates[0].decorate(&upper_fn), "/PROJect/SRC/file.EXT");
        assert_eq!(
            candidates[0].decorate(&|cap: &str| format!("${}$", cap)),
            "/$proj$ect/$src$/file.$ext$"
        );
        assert_eq!(
            candidates[1].decorate(&upper_fn),
            "project/no/match/file.ext"
        );
    }
}

pub struct Menu {
    pub kind: String,
    pub title: String,
    pub entries: Vec<String>,
    pub filter: MenuFilter,
}

impl Menu {
    pub fn new<T>(kind: &str, title: &str, entries: T, search: &str) -> Menu
    where
        T: Into<Vec<String>>,
    {
        Menu {
            kind: kind.to_string(),
            title: title.to_string(),
            entries: entries.into(),
            filter: MenuFilter::new(search),
        }
    }

    pub fn filtered(&self) -> Candidates {
        self.filter.filter(&self.entries)
    }

    pub fn files(search: &str) -> Menu {
        let files: Vec<String> = Walk::new("./")
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| !ft.is_dir()).unwrap_or(false))
            .filter_map(|e| e.path().to_str().map(|s| String::from(&s[2..])))
            .collect();
        Menu::new("files", "file", files, search)
    }
}
