use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::ops::{Deref, Range};

use editor::Buffer;
use remote::protocol::notification::view::{
    Params as NotificationParams, ParamsHeader, ParamsItem, ParamsLines,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Focus {
    Range(Range<usize>),
    Whole,
}

impl Focus {
    pub fn start(&self) -> usize {
        use self::Focus::*;
        match self {
            Range(r) => r.start,
            Whole => 0,
        }
    }
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
            (Focus::Range(a), Focus::Range(b)) => {
                if a.start != b.start {
                    a.start.cmp(&b.start)
                } else {
                    a.end.cmp(&b.end)
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Lens {
    pub buffer: String,
    pub focus: Focus,
}

#[derive(Clone, Debug, Default)]
pub struct LensGroup(Vec<Lens>);

impl LensGroup {
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

#[derive(Clone, Debug, Default)]
pub struct View(BTreeMap<String, LensGroup>);

impl View {
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
            .or_insert_with(LensGroup::default)
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

    pub fn to_notification_params(&self, buffers: &HashMap<String, Buffer>) -> NotificationParams {
        self.as_vec()
            .iter()
            .map(|item| match item {
                ViewItem::Header((buffer, focus)) => {
                    use editor::view::Focus;
                    match focus {
                        Focus::Range(range) => ParamsItem::Header(ParamsHeader {
                            buffer: buffer.to_string(),
                            start: range.start + 1,
                            end: range.end,
                        }),
                        Focus::Whole => {
                            let b = &buffers[&buffer.to_string()];
                            ParamsItem::Header(ParamsHeader {
                                buffer: buffer.to_string(),
                                start: 1,
                                end: b.line_count(),
                            })
                        }
                    }
                }
                ViewItem::Lens(lens) => {
                    let buffer = &buffers[&lens.buffer];
                    ParamsItem::Lines(ParamsLines {
                        lines: buffer.lines(lens.focus.clone()).to_vec(),
                        first_line_num: lens.focus.start() + 1,
                    })
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key() {
        let empty = View::default();
        assert_eq!(empty.key(), "");

        let mut simple = View::default();
        simple.add_lens(Lens {
            buffer: "buffer1".into(),
            focus: Focus::Range(10..12),
        });
        simple.add_lens(Lens {
            buffer: "buffer2".into(),
            focus: Focus::Whole,
        });
        assert_eq!(simple.key(), "buffer1{10..12}|buffer2{*}");

        let mut double = View::default();
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
