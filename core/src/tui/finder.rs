use std::cmp::Ordering;

use regex::Regex;

#[derive(Debug, Eq)]
pub struct Candidate {
    pub text: String,
    score: usize,
}

impl Candidate {
    fn new(re: &Regex, text: &str) -> Candidate {
        let score = re
            .find(&text)
            .map(|m| text.len() - m.start() + m.end())
            .unwrap_or(0);
        Candidate {
            text: text.to_owned(),
            score,
        }
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Candidate) -> Ordering {
        if self == other {
            other.text.cmp(&self.text)
        } else {
            self.score.cmp(&other.score)
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

pub struct Finder {
    re: Regex,
}

impl Finder {
    pub fn new(terms: &str) -> Finder {
        let elements: Vec<String> = terms
            .split_whitespace()
            .map(|e| format!("({})", e))
            .collect();
        let raw_re = format!("(?i){}", elements.join(".*"));
        let re = Regex::new(&raw_re).unwrap();
        Finder { re }
    }

    pub fn search(&mut self, items: &[String]) -> Vec<Candidate> {
        let mut candidates: Vec<Candidate> =
            items.iter().map(|i| Candidate::new(&self.re, i)).collect();
        candidates.sort_by(|a, b| b.cmp(a));
        candidates
    }
}
