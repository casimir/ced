use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::ops::Range;

use crate::editor::selection::Selection;
use crate::editor::Buffer;
use ornament::Decorator;
use remote::protocol::{
    notifications::{ViewParams, ViewParamsItem, ViewParamsLens},
    Face,
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

impl<'a> std::ops::Deref for LensGroup {
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
    pub fn for_buffer(buffer: &str) -> View {
        let mut view = View::default();
        view.add_lens(Lens {
            buffer: buffer.to_string(),
            focus: Focus::Whole,
        });
        view
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
            .or_insert_with(LensGroup::default)
            .add(lens);
    }

    pub fn remove_lens_group(&mut self, buffer: &str) -> Option<LensGroup> {
        self.0.remove(buffer)
    }

    pub fn buffers(&self) -> Vec<&String> {
        self.0.keys().collect()
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

    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
    }

    pub fn to_notification_params(
        &self,
        buffers: &HashMap<String, Buffer>,
        selections: Option<&HashMap<String, Vec<Selection>>>,
    ) -> ViewParams {
        let mut params = Vec::new();
        let mut current = ViewParamsItem::default();
        let mut flush = false;
        for item in self.as_vec() {
            match item {
                ViewItem::Header((buffer, focus)) => {
                    if flush {
                        params.push(current);
                        current = ViewParamsItem::default();
                    }
                    match focus {
                        Focus::Range(range) => {
                            current.buffer = buffer.to_string();
                            current.start = range.start + 1;
                            current.end = range.end;
                        }
                        Focus::Whole => {
                            let b = &buffers[&buffer.to_string()];
                            current.buffer = buffer.to_string();
                            current.start = 1;
                            current.end = b.line_count();
                        }
                    }
                    flush = true;
                }
                ViewItem::Lens(lens) => {
                    let buffer = &buffers[&lens.buffer];
                    let sels = selections.and_then(|ss| ss.get(&lens.buffer));
                    let mut selected: HashMap<usize, Vec<_>> = HashMap::new();
                    if let Some(ss) = sels {
                        for s in ss {
                            let start = buffer.content.offset_to_coord(s.begin()).unwrap();
                            let end = {
                                if s.end() == buffer.content.max_offset() {
                                    let mut end = buffer.content.offset_to_coord(s.end()).unwrap();
                                    end.c += 1;
                                    end
                                } else {
                                    buffer
                                        .content
                                        .navigate(buffer.content.offset_to_coord(s.end()))
                                        .unwrap()
                                        .next()
                                        .pos()
                                        .coords
                                }
                            };
                            if start.l == end.l {
                                let range = (Some(start.c - 1), Some(end.c - 1));
                                selected
                                    .entry(start.l - 1)
                                    .and_modify(|e| e.push(range))
                                    .or_insert_with(|| vec![range]);
                            } else if start.l == end.l - 1 && end.c == 1 {
                                let range = (Some(start.c - 1), None);
                                selected
                                    .entry(start.l - 1)
                                    .and_modify(|e| e.push(range))
                                    .or_insert_with(|| vec![range]);
                            } else {
                                for i in start.l..=end.l {
                                    let range = if i == start.l {
                                        (Some(start.c - 1), None)
                                    } else if i == end.l {
                                        (None, Some(end.c - 1))
                                    } else {
                                        (None, None)
                                    };
                                    selected
                                        .entry(i - 1)
                                        .and_modify(|e| e.push(range))
                                        .or_insert_with(|| vec![range]);
                                }
                            }
                        }
                    }
                    let lines = buffer
                        .lines(lens.focus.clone())
                        .iter()
                        .enumerate()
                        .map(|(i, line)| {
                            let l = line.to_owned() + " ";
                            let mut deco = Decorator::with_text(&l);
                            if let Some(ranges) = selected.get(&i) {
                                for range in ranges {
                                    match *range {
                                        (Some(start), Some(end)) => {
                                            deco.set(Face::Selection, start..end);
                                        }
                                        (Some(start), None) => {
                                            deco.set(Face::Selection, start..l.len());
                                        }
                                        (None, Some(end)) => {
                                            deco.set(Face::Selection, 0..end);
                                        }
                                        (None, None) => {
                                            deco.set(Face::Selection, 0..l.len());
                                        }
                                    }
                                }
                            }
                            deco.build()
                        })
                        .collect();
                    current.lenses.push(ViewParamsLens {
                        lines,
                        first_line_num: lens.focus.start() + 1,
                    });
                }
            }
        }
        if flush {
            params.push(current);
        }
        params
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
