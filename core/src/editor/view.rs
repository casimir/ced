use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::{Deref, Range};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Focus {
    Range(Range<usize>),
    Whole,
}

impl fmt::Display for Focus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Focus::*;
        match self {
            Range(range) => write!(f, "{}..{}", range.start, range.end),
            Whole => write!(f, "*"),
        }
    }
}

impl PartialOrd for Focus {
    fn partial_cmp(&self, other: &Focus) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Focus {
    fn cmp(&self, other: &Focus) -> Ordering {
        match (self, other) {
            (Focus::Whole, _) => Ordering::Greater,
            (_, Focus::Whole) => Ordering::Less,
            (Focus::Range(a), Focus::Range(b)) => if a.start != b.start {
                a.start.cmp(&b.start)
            } else {
                a.end.cmp(&b.end)
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Lens {
    pub buffer: String,
    pub focus: Focus,
}

#[derive(Clone, Debug)]
pub struct LensGroup(Vec<Lens>);

impl LensGroup {
    pub fn new() -> LensGroup {
        LensGroup(Vec::new())
    }

    pub fn add(&mut self, lens: Lens) {
        self.0.push(lens);
        self.0.sort_by(|a, b| a.focus.cmp(&b.focus));
    }

    pub fn focus(&self) -> Focus {
        match (&self.first().unwrap().focus, &self.last().unwrap().focus) {
            (Focus::Whole, _) | (_, Focus::Whole) => Focus::Whole,
            (Focus::Range(first), Focus::Range(last)) => Focus::Range(first.start..last.end),
        }
    }
}

impl<'a> Deref for LensGroup {
    type Target = Vec<Lens>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub enum ViewItem {
    Header((String, Focus)),
    Lens(Lens),
}

#[derive(Clone, Debug)]
pub struct View(BTreeMap<String, LensGroup>);

impl View {
    pub fn new() -> View {
        View(BTreeMap::new())
    }

    pub fn key(&self) -> String {
        let mut parts = Vec::new();
        for (buffer, group) in &self.0 {
            let coords = group
                .iter()
                .map(|lens| lens.focus.to_string())
                .collect::<Vec<String>>();
            parts.push(format!("{}{{{}}}", buffer, coords.join(",")));
        }
        parts.join("|")
    }

    pub fn add_lens(&mut self, lens: Lens) {
        self.0
            .entry(lens.buffer.clone())
            .or_insert_with(LensGroup::new)
            .add(lens);
    }

    pub fn as_vec(&self) -> Vec<ViewItem> {
        let mut list = Vec::new();
        for (buffer, group) in &self.0 {
            list.push(ViewItem::Header((buffer.to_string(), group.focus())));
            for lens in group.iter() {
                list.push(ViewItem::Lens(lens.clone()));
            }
        }
        list
    }

    pub fn contains_buffer(&self, buffer: &str) -> bool {
        self.0.contains_key(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key() {
        let empty = View::new();
        assert_eq!(empty.key(), "");

        let mut simple = View::new();
        simple.add_lens(Lens {
            buffer: "buffer1".into(),
            focus: Focus::Range(10..12),
        });
        simple.add_lens(Lens {
            buffer: "buffer2".into(),
            focus: Focus::Whole,
        });
        assert_eq!(simple.key(), "buffer1{10..12}|buffer2{*}");

        let mut double = View::new();
        double.add_lens(Lens {
            buffer: "buffer1".into(),
            focus: Focus::Range(10..12),
        });
        double.add_lens(Lens {
            buffer: "buffer1".into(),
            focus: Focus::Range(20..51),
        });
        double.add_lens(Lens {
            buffer: "buffer2".into(),
            focus: Focus::Whole,
        });
        assert_eq!(double.key(), "buffer1{10..12,20..51}|buffer2{*}");
    }
}
