use std::cmp::Ordering;
use std::fmt;
use std::ops::Deref;

use regex::{CaptureLocations, Regex};

use editor::Editor;
use remote::protocol::notification::menu::{Entry, Params as NotificationParams};
use remote::protocol::{Face, TextFragment};

#[derive(Debug, Eq, PartialEq)]
pub struct Token {
    pub text: String,
    pub is_match: bool,
}

pub trait Searchable {
    fn field(&self) -> &str;
}

impl Searchable for String {
    fn field(&self) -> &str {
        &self
    }
}

fn compute_candidate_score(locations: &CaptureLocations) -> f32 {
    if locations.len() > 0 {
        let (start, _) = locations.get(0).unwrap();
        let (_, end) = locations.get(locations.len() - 1).unwrap();
        100.0 / (1 + end - start) as f32
    } else {
        0.0
    }
}

#[derive(Debug)]
pub struct Candidate<T: Searchable> {
    pub object: T,
    score: Option<f32>,
    locations: CaptureLocations,
}

impl<T: Searchable> Candidate<T> {
    fn new(re: &Regex, object: T) -> Candidate<T> {
        let mut locations = re.capture_locations();
        let score = re
            .captures_read(&mut locations, object.field())
            .map(|_| compute_candidate_score(&locations));
        Candidate {
            object,
            score,
            locations,
        }
    }

    pub fn is_match(&self) -> bool {
        self.score.is_some()
    }

    pub fn tokenize(&self) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut last_end = 0;
        let text = self.object.field();
        for i in 1..self.locations.len() {
            if let Some((start, end)) = self.locations.get(i) {
                tokens.push(Token {
                    text: text[last_end..start].to_owned(),
                    is_match: false,
                });
                tokens.push(Token {
                    text: text[start..end].to_owned(),
                    is_match: true,
                });
                last_end = end;
            }
        }
        if last_end < text.len() {
            tokens.push(Token {
                text: text[last_end..].to_owned(),
                is_match: false,
            });
        }
        tokens
    }
}

impl<T: Searchable> fmt::Display for Candidate<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:.2} {}",
            self.score.unwrap_or(-1.0),
            self.object.field()
        )
    }
}

impl<T: Searchable> Ord for Candidate<T> {
    fn cmp(&self, other: &Candidate<T>) -> Ordering {
        if self == other {
            other.object.field().cmp(&self.object.field())
        } else if self.score > other.score {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }
}

impl<T: Searchable> PartialOrd for Candidate<T> {
    fn partial_cmp(&self, other: &Candidate<T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Searchable> PartialEq for Candidate<T> {
    fn eq(&self, other: &Candidate<T>) -> bool {
        self.score == other.score
    }
}

impl<T: Searchable> Eq for Candidate<T> {}

#[derive(Default)]
pub struct Candidates<T: Searchable>(Vec<Candidate<T>>);

impl<T: Searchable> Deref for Candidates<T> {
    type Target = Vec<Candidate<T>>;

    fn deref(&self) -> &Self::Target {
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

    pub fn filter<T>(&self, items: &[T]) -> Candidates<T>
    where
        T: Searchable + Clone,
    {
        let re = self.build_regex();
        let mut candidates: Vec<Candidate<T>> = items
            .iter()
            .map(|ref i| Candidate::new(&re, (*i).clone()))
            .collect();
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
        let candidates: Candidates<String> = f.filter(&items);
        let res: Vec<String> = candidates
            .iter()
            .map(|ref c| c.object.field().to_string())
            .collect();
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
        let candidates1: Candidates<String> = f.filter(&items1);
        let res1: Vec<String> = candidates1
            .iter()
            .map(|ref c| c.object.field().to_string())
            .collect();

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
        let candidates2: Candidates<String> = f.filter(&items2);
        let res2: Vec<String> = candidates2
            .iter()
            .map(|ref c| c.object.field().to_string())
            .collect();

        assert_eq!(res1, expected1);
        assert_eq!(res2, expected2);
    }

    #[test]
    fn test_tokenize() {
        let items = vec![
            "/project/src/file.ext".to_owned(),
            "project/no/match/file.ext".to_owned(),
        ];
        let candidates = MenuFilter::new("proj src ext").filter(&items);
        assert_eq!(
            candidates[0].tokenize(),
            vec![
                Token {
                    text: "/".to_owned(),
                    is_match: false
                },
                Token {
                    text: "proj".to_owned(),
                    is_match: true
                },
                Token {
                    text: "ect/".to_owned(),
                    is_match: false
                },
                Token {
                    text: "src".to_owned(),
                    is_match: true
                },
                Token {
                    text: "/file.".to_owned(),
                    is_match: false
                },
                Token {
                    text: "ext".to_owned(),
                    is_match: true
                },
            ]
        );
        assert_eq!(
            candidates[1].tokenize(),
            vec![Token {
                text: "project/no/match/file.ext".to_owned(),
                is_match: false
            }],
        );
    }
}

pub type MenuAction = fn(&str, &mut Editor, usize) -> Result<(), failure::Error>;

#[derive(Clone)]
pub struct MenuEntry {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub action: MenuAction,
}

impl Searchable for MenuEntry {
    fn field(&self) -> &str {
        &self.label
    }
}

pub type EntryProvider = fn() -> Vec<MenuEntry>;

#[derive(Clone)]
pub struct Menu {
    pub command: String,
    pub title: String,
    provider: EntryProvider,
    entries: Vec<MenuEntry>,
}

impl Menu {
    pub fn new(command: &str, title: &str, provider: EntryProvider) -> Menu {
        let mut menu = Menu {
            command: command.to_string(),
            title: title.to_string(),
            provider,
            entries: Vec::new(),
        };
        menu.populate();
        menu
    }

    pub fn populate(&mut self) {
        self.entries = (self.provider)();
    }

    pub fn get(&self, key: &str) -> Option<&MenuEntry> {
        self.entries.iter().find(|e| e.key == key)
    }

    pub fn filter(&self, search: &str) -> Candidates<MenuEntry> {
        let filter = MenuFilter::new(search);
        filter.filter(&self.entries)
    }

    pub fn to_notification_params(&self, search: &str) -> NotificationParams {
        let command = self.command.to_string();
        let title = self.title.to_string();
        let entries = self
            .filter(search)
            .iter()
            .filter(|c| c.is_match())
            .map(|c| Entry {
                value: c.object.key.clone(),
                fragments: c
                    .tokenize()
                    .iter()
                    .map(|t| TextFragment {
                        text: t.text.clone(),
                        face: if t.is_match {
                            Face::Match
                        } else {
                            Face::Default
                        },
                    }).collect(),
                description: c.object.description.clone(),
            }).collect();
        NotificationParams {
            command,
            title,
            search: search.to_string(),
            entries,
        }
    }
}
