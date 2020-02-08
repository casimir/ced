mod position;

use std::cmp::{max, Ordering};
use std::collections::BTreeSet;
use std::iter::FromIterator;

use crate::editor::diff::{diff, Diff};
use crate::editor::range::{OffsetRange, Range};
pub use position::PositionIterator;
use rbtset::{Consecutive, Node, RBTreeSet};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, Eq)]
struct Piece {
    offset: usize,
    start: usize,
    length: usize,
    original: bool,
}

impl Piece {
    fn offset(offset: usize) -> Piece {
        Piece {
            offset,
            start: 0,
            length: 0,
            original: false,
        }
    }

    #[inline]
    fn end(&self) -> usize {
        self.start + self.length
    }

    #[inline]
    fn contains(&self, offset: usize) -> bool {
        self.offset <= offset && offset < self.offset + self.length
    }

    fn split(self, offset: usize, length: usize) -> (Option<Piece>, Piece) {
        if offset == self.offset {
            let p2 = Piece {
                offset: offset + length,
                ..self
            };
            (None, p2)
        } else {
            let p1 = Piece {
                length: offset - self.offset,
                ..self
            };
            let p2 = Piece {
                offset: offset + length,
                start: p1.end(),
                length: self.length - p1.length,
                ..self
            };
            (Some(p1), p2)
        }
    }

    fn truncate(self, offset: usize, length: usize) -> (Option<Piece>, Option<Piece>) {
        let p1 = if self.contains(offset) && self.offset != offset {
            Some(Piece {
                length: offset - self.offset,
                ..self
            })
        } else {
            None
        };
        let end = offset + length;
        let p2 = if self.contains(end) {
            Some(Piece {
                offset,
                start: self.start + end - self.offset,
                length: self.length - (end - self.offset),
                ..self
            })
        } else {
            None
        };
        (p1, p2)
    }

    fn ranged(self, range: OffsetRange) -> Piece {
        if self.contains(range.start()) && self.contains(range.end()) {
            Piece {
                offset: range.start(),
                start: self.start + range.start() - self.offset,
                length: range.len(),
                ..self
            }
        } else if self.contains(range.start()) {
            Piece {
                offset: range.start(),
                start: self.start + range.start() - self.offset,
                length: self.length - (range.start() - self.offset),
                ..self
            }
        } else if self.contains(range.end()) {
            Piece {
                length: range.end() - self.offset,
                ..self
            }
        } else {
            self
        }
    }
}

impl Ord for Piece {
    fn cmp(&self, other: &Piece) -> Ordering {
        self.offset.cmp(&other.offset)
    }
}

impl PartialOrd for Piece {
    fn partial_cmp(&self, other: &Piece) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Piece {
    fn eq(&self, other: &Piece) -> bool {
        other.offset <= self.offset && self.offset < other.offset + other.length
    }
}

impl Consecutive for Piece {
    fn consecutive(&self, other: &Piece) -> bool {
        self.offset + self.length == other.offset
            && self.original == other.original
            && self.end() == other.start
    }

    fn merged(&self, other: &Piece) -> Piece {
        Piece {
            length: self.length + other.length,
            ..*self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Bulk,
    Delete,
    Insert,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coords {
    pub l: usize,
    pub c: usize,
}

impl From<(usize, usize)> for Coords {
    fn from(tuple: (usize, usize)) -> Coords {
        Coords {
            l: tuple.0,
            c: tuple.1,
        }
    }
}

pub struct PieceTable {
    original: Vec<u8>,
    added: Vec<u8>,
    pieces: RBTreeSet<Piece>,
    newlines: BTreeSet<usize>,
    last_action: Option<Action>,
    undos: Vec<RBTreeSet<Piece>>,
    redos: Vec<RBTreeSet<Piece>>,
}

impl PieceTable {
    pub fn with_text(text: String) -> PieceTable {
        let mut pieces = RBTreeSet::new();
        let newlines = text.match_indices('\n').map(|(i, _)| i).collect();
        pieces.insert(Piece {
            offset: 0,
            start: 0,
            length: text.len(),
            original: true,
        });
        PieceTable {
            original: text.into_bytes(),
            added: Vec::new(),
            pieces,
            newlines,
            last_action: None,
            undos: Vec::new(),
            redos: Vec::new(),
        }
    }

    pub fn new() -> PieceTable {
        PieceTable {
            original: Vec::new(),
            added: Vec::new(),
            pieces: RBTreeSet::new(),
            newlines: BTreeSet::new(),
            last_action: None,
            undos: Vec::new(),
            redos: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.pieces.values().fold(0, |acc, p| acc + p.length)
    }

    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    fn commit(&mut self) {
        self.pieces.repack();
        self.undos.push(self.pieces.clone());
        self.redos.clear();
        self.last_action = None;
    }

    fn action(&mut self, action: Action) {
        if self.last_action == Some(Action::Bulk) {
            return;
        }
        if self.last_action != Some(action) {
            self.commit();
            self.last_action = Some(action);
        }
    }

    fn start_bulk(&mut self) {
        self.action(Action::Bulk);
    }

    fn end_bulk(&mut self) {
        self.last_action = None;
    }

    fn undo(&mut self) -> bool {
        if let Some(pieces) = self.undos.pop() {
            self.redos.push(self.pieces.clone());
            self.pieces = pieces;
            self.last_action = None;
            true
        } else {
            false
        }
    }

    fn redo(&mut self) -> bool {
        if let Some(pieces) = self.redos.pop() {
            self.undos.push(self.pieces.clone());
            self.pieces = pieces;
            self.last_action = None;
            true
        } else {
            false
        }
    }

    fn range(&self, range: OffsetRange) -> Option<Vec<u8>> {
        self.pieces
            .get_node(&Piece::offset(range.start()))
            .map(|start_piece| {
                self.pieces
                    .values_from(&start_piece)
                    .take_while(|p| p.offset < range.end())
                    .map(|p| {
                        let buffer = if p.original {
                            &self.original
                        } else {
                            &self.added
                        };
                        let ranged_piece = p.ranged(range);
                        &buffer[ranged_piece.start..ranged_piece.end()]
                    })
                    .collect::<Vec<&[u8]>>()
                    .concat()
            })
    }

    fn join(&self, sep: &str) -> String {
        self.pieces
            .values()
            .map(|p| {
                let buffer = if p.original {
                    &self.original
                } else {
                    &self.added
                };
                String::from_utf8_lossy(&buffer[p.start..p.end()]).into()
            })
            .collect::<Vec<String>>()
            .join(sep)
    }

    pub fn text(&self) -> String {
        self.join("")
    }

    pub fn text_range(&self, range: OffsetRange) -> Option<String> {
        // TODO handle decoding error
        self.range(range)
            .map(|bs| String::from_utf8_lossy(&bs).into())
    }

    pub fn lines(&self) -> Vec<String> {
        self.text().lines().map(ToOwned::to_owned).collect()
    }

    pub fn line_count(&self) -> usize {
        self.newlines.len() + 1
    }

    pub fn line(&self, n: usize) -> Option<String> {
        if n == 0 || self.line_count() < n {
            None
        } else if n == 1 {
            let max_len = self.len();
            self.text_range(OffsetRange::new(
                0,
                *self.newlines.iter().nth(0).unwrap_or(&max_len),
            ))
        } else {
            let start = self.newlines.iter().nth(n - 2).unwrap();
            let end = match self.newlines.iter().nth(n - 1) {
                Some(&v) => v - 1,
                None => self.len(),
            };
            self.text_range(OffsetRange::new(start + 1, end - start))
        }
    }

    pub fn offset_to_coord(&self, offset: usize) -> Coords {
        // start
        if offset == 0 {
            return Coords { c: 1, l: 1 };
        }

        // bigger that total length
        if self.len() <= offset {
            let lineno = self.newlines.len() + 1;
            return Coords {
                l: lineno,
                c: self.line_length(lineno) + 1,
            };
        }

        // points to a newline
        if let Some(idx) = self.newlines.iter().position(|&x| x == offset) {
            let lineno = idx + 1;
            return Coords {
                l: lineno,
                c: self.line_length(lineno) + 1,
            };
        }

        let preceding_lines = self.newlines.range(..offset).collect::<Vec<_>>();
        let lineno = preceding_lines.len() + 1;
        match preceding_lines.last() {
            Some(&nli) => {
                let line = self.line(lineno).unwrap();
                let indices = BTreeSet::from_iter(line.grapheme_indices(true).map(|(i, _)| i));
                let col = indices.range(..offset - nli).count();
                Coords { l: lineno, c: col }
            }
            None => Coords {
                l: lineno,
                c: offset + 1,
            },
        }
    }

    pub fn coord_to_offset(&self, coords: Coords) -> usize {
        if coords.l == 0 {
            0
        } else if self.line_count() < coords.l {
            self.len()
        } else {
            let line_offset = if coords.l > 1 {
                self.newlines
                    .iter()
                    .nth(coords.l - 2)
                    .map_or(0, |nl| nl + 1)
            } else {
                0
            };
            let col_offset = self
                .line(coords.l)
                .and_then(|l| l.grapheme_indices(true).nth(coords.c - 1).map(|(o, _)| o))
                .unwrap_or_else(|| self.line_length(coords.l));
            line_offset + col_offset
        }
    }

    pub fn char_at_offset(&self, offset: usize) -> Option<String> {
        // TODO the closest new line is used but this could be optimized using the next boundary
        let end = self
            .newlines
            .range(offset..)
            .nth(0)
            .map_or_else(|| self.len(), |&nl| nl);
        self.text_range(OffsetRange::new(offset, max(end - offset, 1)))
            .and_then(|l| l.graphemes(true).nth(0).map(ToOwned::to_owned))
    }

    pub fn char_at<C: Into<Coords>>(&self, coords: C) -> Option<String> {
        self.char_at_offset(self.coord_to_offset(coords.into()))
    }

    pub fn line_length(&self, index: usize) -> usize {
        let mut nls = self.newlines.iter().collect::<Vec<_>>();
        nls.sort_unstable();
        if nls.is_empty() {
            self.len()
        } else if index == 1 {
            *nls[0]
        } else if index - 1 < nls.len() {
            nls[index - 1] - nls[index - 2] - 1
        } else if index - 1 == nls.len() {
            self.len() - nls[index - 2] - 1
        } else {
            0
        }
    }

    pub fn longest_line(&self) -> (usize, usize) {
        let mut longest = (0, 0);
        for i in 1..=(self.newlines.len() + 1) {
            let len = self.line_length(i);
            if longest.1 < len {
                longest = (i, len);
            }
        }
        longest
    }

    pub fn lines_range<C: Into<Coords>>(&self, from: C, to: C) -> Vec<String> {
        let offset = self.coord_to_offset(from.into());
        let length = self.coord_to_offset(to.into()) - offset;
        let range = OffsetRange::new(offset, length);
        self.text_range(range)
            .unwrap_or_default()
            .lines()
            .map(ToOwned::to_owned)
            .collect()
    }

    pub fn append(&mut self, text: String) {
        self.action(Action::Insert);
        let offset = if let Some(last) = self.pieces.last() {
            let data = last.data();
            data.offset + data.length
        } else {
            0
        };
        let index = self.added.len();
        self.pieces.insert(Piece {
            offset,
            start: index,
            length: text.len(),
            original: false,
        });
        self.newlines
            .extend(text.match_indices('\n').map(|(i, _)| offset + i));
        self.added.extend(text.into_bytes());
    }

    fn shift_offset_after(&mut self, node: &Node<Piece>, value: i64) {
        if value != 0 {
            for n in self.pieces.iter_from(node).skip(1) {
                n.apply(|p| p.offset = (p.offset as i64 + value) as usize);
            }
        }
    }

    pub fn insert(&mut self, offset: usize, text: String) {
        self.action(Action::Insert);
        if let Some(ref mut node) = self.pieces.get_node(&Piece::offset(offset)) {
            let added_start = self.added.len();
            self.added.extend(text.as_bytes());
            let new = Piece {
                offset,
                start: added_start,
                length: text.len(),
                original: false,
            };
            let sub_pieces = node.data().split(new.offset, new.length);
            self.shift_offset_after(node, new.length as i64);
            self.pieces.remove_node(node);
            if let Some(p) = sub_pieces.0 {
                self.pieces.insert(p);
            }
            self.pieces.insert(new);
            self.pieces.insert(sub_pieces.1);
            self.newlines = self
                .newlines
                .iter()
                .map(|&i| if offset <= i { i + new.length } else { i })
                .collect();
            self.newlines.extend(
                text.match_indices('\n')
                    .map(|(i, _)| i + offset)
                    .collect::<Vec<_>>(),
            );
        } else {
            self.append(text);
        }
    }

    pub fn delete(&mut self, range: OffsetRange) {
        self.action(Action::Delete);
        if let Some(start_node) = self.pieces.get_node(&Piece::offset(range.start())) {
            let pieces = self
                .pieces
                .values_from(&start_node)
                .take_while(|p| p.offset < range.end())
                .collect::<Vec<Piece>>();
            let (head, _) = pieces[0].truncate(range.start(), range.len());
            let (_, tail) = pieces[pieces.len() - 1].truncate(range.start(), range.len());
            for piece in &pieces {
                assert!(self.pieces.remove(piece));
            }
            let mut last_node = None;
            if let Some(p) = head {
                last_node = self.pieces.insert(p);
            }
            if let Some(p) = tail {
                last_node = self.pieces.insert(p);
            }
            if let Some(ln) = last_node {
                self.shift_offset_after(&ln, -(range.len() as i64));
            } else {
                start_node.apply(|n| n.offset -= range.len());
                self.shift_offset_after(&start_node, -(range.len() as i64));
            }
            self.newlines = self
                .newlines
                .iter()
                .filter_map(|&i| {
                    if range.start() <= i && i < range.end() {
                        None
                    } else if range.end() <= i {
                        Some(i - range.len())
                    } else {
                        Some(i)
                    }
                })
                .collect();
        }
    }

    pub fn replace(&mut self, range: OffsetRange, text: String) {
        self.start_bulk();
        self.delete(range);
        self.insert(range.start(), text);
        self.end_bulk();
    }

    pub fn apply_diff(&mut self, text: &str) {
        let original = self.text();
        // TODO trailing newline optimization
        let diffs = diff(&original, text);
        let mut loffset = 0;
        let mut roffset = 0;

        self.start_bulk();
        for diff in diffs {
            match diff {
                Diff::Left(len) => self.delete(OffsetRange::new(loffset, len)),
                Diff::Right(len) => {
                    self.insert(loffset, text[roffset..roffset + len].to_owned());
                    loffset += len;
                    roffset += len;
                }
                Diff::Both(len) => {
                    loffset += len;
                    roffset += len;
                }
            }
        }
        self.end_bulk();
    }
}

impl Default for PieceTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert() {
        let mut pieces = PieceTable::new();
        pieces.insert(0, "the fox jumps over the dog".to_owned());
        pieces.insert(4, "quick brown ".to_owned());
        pieces.insert(35, "lazy ".to_owned());
        pieces.append(" 🐶".to_owned());
        pieces.insert(0, "🦊 ".to_owned());
        pieces.insert(56, ", so quick".to_owned());

        print!("{}", pieces.pieces.dump_tree_as_dot());
        assert_eq!(
            pieces.text(),
            "🦊 the quick brown fox jumps over the lazy dog 🐶, so quick"
        );
    }

    #[test]
    fn delete() {
        let mut pieces = PieceTable::new();
        pieces.insert(0, "the fox jumps over the dog".to_owned());
        pieces.insert(3, " quick brown".to_owned());
        pieces.insert(35, "lazy ".to_owned());
        pieces.append(" 🐶".to_owned());
        pieces.insert(0, "🦊 ".to_owned());
        pieces.insert(56, ", so quick".to_owned());

        assert_eq!(
            pieces.text_range(OffsetRange::new(9, 12)),
            Some(String::from("quick brown "))
        );
        pieces.delete(OffsetRange::new(9, 12)); // "quick brown| "
        assert_eq!(
            pieces.text_range(OffsetRange::new(28, 5)),
            Some(String::from("lazy "))
        );
        pieces.delete(OffsetRange::new(28, 5)); // "|lazy |"

        print!("{}", pieces.pieces.dump_tree_as_dot());
        assert_eq!(pieces.text(), "🦊 the fox jumps over the dog 🐶, so quick");
    }

    #[test]
    fn replace() {
        let mut pieces = PieceTable::new();
        pieces.insert(0, "the fox jumps over the dog".to_owned());
        pieces.insert(4, "quick brown ".to_owned());
        pieces.insert(35, "lazy ".to_owned());
        pieces.append(" 🐶".to_owned());
        pieces.insert(0, "🦊 ".to_owned());
        pieces.insert(56, ", so quick".to_owned());

        pieces.replace(OffsetRange::new(9, 11), "sneaky".to_owned()); // "quick brown| "
        pieces.replace(OffsetRange::new(35, 8), "mighty bear".to_owned()); // "|lazy |dog|"

        print!("{}", pieces.pieces.dump_tree_as_dot());
        assert_eq!(
            pieces.text(),
            "🦊 the sneaky fox jumps over the mighty bear 🐶, so quick"
        );
    }

    #[test]
    fn apply_diff() {
        let mut pieces = PieceTable::new();
        pieces.insert(0, "the fox jumps over the dog".to_owned());
        pieces.insert(4, "quick brown ".to_owned());
        pieces.insert(35, "lazy ".to_owned());
        pieces.append(" 🐶".to_owned());
        pieces.insert(0, "🦊 ".to_owned());
        pieces.insert(56, ", so quick".to_owned());

        let new_text = "🦊 the sneaky fox jumps over the mighty bear 🐶, so quick";
        pieces.apply_diff(new_text);

        print!("{}", pieces.pieces.dump_tree_as_dot());
        assert_eq!(pieces.text(), new_text);
    }

    #[test]
    fn consecutive() {
        let mut pieces = PieceTable::new();
        pieces.append("Where is".to_owned());
        pieces.append(" ".to_owned());
        pieces.append("my mind".to_owned());
        pieces.append("?".to_owned());
        assert_eq!(pieces.text(), "Where is my mind?");
        assert_eq!(pieces.pieces.len(), 4);

        pieces.pieces.repack();
        assert_eq!(pieces.text(), "Where is my mind?");
        assert_eq!(pieces.pieces.len(), 1);

        pieces.insert(0, "Hey. ".to_owned());
        pieces.pieces.repack();
        assert_eq!(pieces.text(), "Hey. Where is my mind?");
        assert_eq!(pieces.pieces.len(), 2);
    }

    #[test]
    fn undo_redo() {
        let text = "🦊 the quick brown fox jumps over the lazy dog 🐶, so quick";
        let mut pieces = PieceTable::with_text(text.to_owned());

        // empty undo stack
        assert!(!pieces.undo());

        // delete + undo/redo
        pieces.delete(OffsetRange::new(53, 10));
        let deleted = "🦊 the quick brown fox jumps over the lazy dog 🐶";
        assert_eq!(pieces.text(), deleted);
        assert!(pieces.undo());
        assert!(!pieces.undo());
        assert_eq!(pieces.text(), text);
        assert!(pieces.redo());
        assert!(!pieces.redo());
        assert_eq!(pieces.text(), deleted);

        // insert + insert + delete + undo/redo
        let inserted = "🦊 the really quick brown fox jumps over the lazy dog 🐶, so fast";
        pieces.append(", so fast".to_owned());
        pieces.insert(9, "really ".to_owned());
        assert_eq!(pieces.text(), inserted);
        let redeleted = "the really quick brown fox jumps over the lazy dog 🐶, so fast";
        pieces.delete(OffsetRange::new(0, 5));
        assert_eq!(pieces.text(), redeleted);
        assert!(pieces.undo());
        assert_eq!(pieces.text(), inserted);
        assert!(pieces.undo());
        assert_eq!(pieces.text(), deleted);
        assert!(pieces.redo());
        assert!(pieces.redo());
        assert_eq!(pieces.text(), redeleted);

        // bulk + undo/redo
        let new_text = "nothing to see here";
        pieces.apply_diff(new_text);
        assert_eq!(pieces.text(), new_text);
        assert!(pieces.undo());
        assert_eq!(pieces.text(), redeleted);
        assert!(pieces.redo());
        assert_eq!(pieces.text(), new_text);
    }

    #[test]
    fn newline_caching() {
        let mut pieces = PieceTable::new();
        assert!(pieces.newlines.is_empty());
        pieces.append("first line\nsecond line\nthe end".to_owned());
        assert_eq!(
            pieces.newlines,
            [10, 22].iter().map(|&i| i as usize).collect()
        );
        pieces.insert(11, "surprise line\n".to_owned());
        assert_eq!(
            pieces.newlines,
            [10, 24, 36].iter().map(|&i| i as usize).collect()
        );
        pieces.replace(OffsetRange::new(11, 8), "another".to_owned());
        assert_eq!(
            pieces.newlines,
            [10, 23, 35].iter().map(|&i| i as usize).collect()
        );
        pieces.delete(OffsetRange::new(11, 24));
        assert_eq!(
            pieces.newlines,
            [10, 11].iter().map(|&i| i as usize).collect()
        );
    }

    #[test]
    fn line_length() {
        let pieces = PieceTable::with_text("first line\nsecond line\nthe end".to_owned());
        assert_eq!(pieces.line_length(1), 10);
        assert_eq!(pieces.line_length(2), 11);
        assert_eq!(pieces.line_length(3), 7);
        assert_eq!(pieces.line_length(4), 0);

        assert_eq!(pieces.longest_line(), (2, 11));
    }

    #[test]
    fn line() {
        let pieces = PieceTable::with_text(
            "What does the 🦊 says?\n❓ It's a mystery.\nA real mystery.".to_owned(),
        );
        assert_eq!(pieces.line(0), None);
        assert_eq!(pieces.line(1), Some("What does the 🦊 says?".into()));
        assert_eq!(pieces.line(2), Some("❓ It's a mystery.".into()));
        assert_eq!(pieces.line(3), Some("A real mystery.".into()));
        assert_eq!(pieces.line(10), None);
    }

    #[test]
    fn coordinates() {
        let text = r#"Vous qui venez ici
dans une humble posture

De vos flancs alourdis
décharger le fardeau

Veuillez quand vous aurez
Soulagé la nature

Et déposé dans l'urne
un modeste cadeau

Epancher dans l'amphore
un courant d'onde pure

Et sur l'autel fumant
placer pour chapiteau

Le couvercle arrondi
dont l'auguste jointure

Aux parfums indiscrets
doit servir de tombeau"#;

        let pieces = PieceTable::with_text(text.to_owned());
        assert_eq!(
            pieces.lines_range((1, 6), (1, 15)),
            vec!["qui venez".to_owned()]
        );
        assert_eq!(
            pieces.lines_range((1, 1), (2, 1)),
            vec!["Vous qui venez ici".to_owned()]
        );
        assert_eq!(
            pieces.lines_range((4, 1), (6, 1)),
            vec![
                "De vos flancs alourdis".to_owned(),
                "décharger le fardeau".to_owned(),
            ]
        );
        assert_eq!(
            pieces.lines_range((7, 10), (8, 8)),
            vec!["quand vous aurez".to_owned(), "Soulagé".to_owned()]
        );

        let coord = (4, 15).into(); // after only 1-len chars
        assert_eq!(pieces.offset_to_coord(pieces.coord_to_offset(coord)), coord);
        let coord = (5, 5).into(); // after 2-len char (é)
        assert_eq!(pieces.offset_to_coord(pieces.coord_to_offset(coord)), coord);

        assert_eq!(
            pieces.offset_to_coord(pieces.coord_to_offset((3, 999).into())),
            Coords { l: 3, c: 1 }
        );

        assert_eq!(pieces.coord_to_offset((9999, 9999).into()), pieces.len());
    }

    #[test]
    fn coordinates_one_line() {
        let pieces = PieceTable::with_text("What does the fox says?".to_owned());
        assert_eq!(pieces.coord_to_offset((1, 5).into()), 4);
        assert_eq!(pieces.coord_to_offset((1, 15).into()), 14);
    }

    #[test]
    fn coordinates_unicode() {
        let pieces = PieceTable::with_text("What does the 🦊 says?\n❓ It's a mystery.".to_owned());
        assert_eq!(pieces.coord_to_offset((1, 5).into()), 4);
        assert_eq!(pieces.coord_to_offset((1, 15).into()), 14); // 🦊 offset
        assert_eq!(pieces.coord_to_offset((1, 16).into()), 18); // after 🦊
        assert_eq!(pieces.coord_to_offset((2, 3).into()), 29);
        assert_eq!(pieces.coord_to_offset((3, 14).into()), pieces.len());
    }

    #[test]
    fn char_at() {
        let text = r#"Natoque.
Ullamcorper ultrices 💘🐬🌜👤🎑🍶🔃 eget accumsan ipsum nunc est eget 🔔🏇💡🔈👝🐚👓🔯🎦👯🍭 🍪🐯🍵 feugiat eget enim, 💉🎽🐂🎂 mauris nisi, non at 🔬👨🔀 odio.
Volutpat massa, et sit aliquam vestibulum, eu nisl rhoncus, commodo, at ac tempor, neque, congue aliquam quam nulla sit nisl sed.
Dolor."#;

        let pieces = PieceTable::with_text(text.to_owned());
        assert_eq!(pieces.char_at((1, 3)), Some("t".into()));
        assert_eq!(pieces.char_at((2, 20)), Some("s".into()));
        assert_eq!(pieces.char_at((2, 22)), Some("💘".into()));
        assert_eq!(pieces.char_at((2, 24)), Some("🌜".into()));
        assert_eq!(pieces.char_at((3, 20)), Some("s".into()));
        assert_eq!(pieces.char_at((4, 2)), Some("o".into()));

        assert_eq!(pieces.char_at((1, 9999)), Some("\n".into()));
        assert_eq!(pieces.char_at((4, 9999)), None);
        assert_eq!(pieces.char_at((5, 1)), None);
        assert_eq!(pieces.char_at_offset(pieces.len()), None);
    }
}
