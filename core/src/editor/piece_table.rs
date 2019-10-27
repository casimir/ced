use std::cmp::{max, min, Ordering};
use std::collections::BTreeSet;

use crate::editor::diff::{diff, Diff};
use crate::editor::range::{OffsetRange, Range};
use rbtset::{Consecutive, Node, RBTreeSet};

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
pub struct Coord {
    pub l: usize,
    pub c: usize,
}

impl From<(usize, usize)> for Coord {
    fn from(tuple: (usize, usize)) -> Coord {
        Coord {
            l: tuple.0,
            c: tuple.1,
        }
    }
}

pub struct PieceTable {
    original: String,
    added: String,
    pieces: RBTreeSet<Piece>,
    newlines: BTreeSet<usize>,
    last_action: Option<Action>,
    undos: Vec<RBTreeSet<Piece>>,
    redos: Vec<RBTreeSet<Piece>>,
}

impl PieceTable {
    pub fn with_text(text: &str) -> PieceTable {
        let mut pieces = RBTreeSet::new();
        let newlines = text.match_indices('\n').map(|(i, _)| i).collect();
        pieces.insert(Piece {
            offset: 0,
            start: 0,
            length: text.len(),
            original: true,
        });
        PieceTable {
            original: String::from(text),
            added: String::new(),
            pieces,
            newlines,
            last_action: None,
            undos: Vec::new(),
            redos: Vec::new(),
        }
    }

    pub fn new() -> PieceTable {
        PieceTable {
            original: String::new(),
            added: String::new(),
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

    fn join(&self, sep: &str) -> String {
        self.pieces
            .values()
            .map(|p| {
                let buffer = if p.original {
                    &self.original
                } else {
                    &self.added
                };
                String::from(&buffer[p.start..p.end()])
            })
            .collect::<Vec<String>>()
            .join(sep)
    }

    pub fn text(&self) -> String {
        self.join("")
    }

    pub fn text_range(&self, range: OffsetRange) -> Option<String> {
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
                        String::from(&buffer[ranged_piece.start..ranged_piece.end()])
                    })
                    .collect::<Vec<String>>()
                    .join("")
            })
    }

    pub fn lines(&self) -> Vec<String> {
        self.text().lines().map(ToOwned::to_owned).collect()
    }

    pub fn line_count(&self) -> usize {
        self.newlines.len() + 1
    }

    pub fn offset_to_coord(&self, offset: usize) -> Coord {
        let offset = min(offset, self.len());
        let preceding_lines = self.newlines.range(..offset).collect::<Vec<_>>();
        let line_offset = preceding_lines.last().map_or(0, |&&i| i);
        Coord {
            l: preceding_lines.len() + 1,
            c: max(offset - line_offset, 1),
        }
    }

    pub fn coord_to_offset(&self, coord: Coord) -> usize {
        if coord.l == 1 {
            min(coord.c - 1, self.line_length(1))
        } else if coord.l - 2 <= self.newlines.len() {
            let mut nls = self.newlines.iter().collect::<Vec<_>>();
            nls.sort_unstable();
            let max_col = if coord.l <= nls.len() {
                *nls[coord.l - 1]
            } else {
                self.len()
            };
            min(*nls[coord.l - 2] + coord.c, max_col)
        } else {
            self.len()
        }
    }

    pub fn line_length(&self, index: usize) -> usize {
        let mut nls = self.newlines.iter().collect::<Vec<_>>();
        nls.sort_unstable();
        if nls.is_empty() {
            0
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

    pub fn lines_range<C>(&self, from: C, to: C) -> Vec<String>
    where
        C: Into<Coord>,
    {
        let offset = self.coord_to_offset(from.into());
        let length = self.coord_to_offset(to.into()) - offset;
        let range = OffsetRange::new(offset, length);
        self.text_range(range)
            .unwrap_or_default()
            .lines()
            .map(ToOwned::to_owned)
            .collect()
    }

    pub fn append(&mut self, text: &str) {
        self.action(Action::Insert);
        let offset = if let Some(last) = self.pieces.last() {
            let data = last.data();
            data.offset + data.length
        } else {
            0
        };
        let index = self.added.len();
        self.added.push_str(text);
        self.pieces.insert(Piece {
            offset,
            start: index,
            length: text.len(),
            original: false,
        });
        self.newlines
            .extend(text.match_indices('\n').map(|(i, _)| offset + i))
    }

    fn shift_offset_after(&mut self, node: &Node<Piece>, value: i64) {
        if value != 0 {
            for n in self.pieces.iter_from(node).skip(1) {
                n.apply(|p| p.offset = (p.offset as i64 + value) as usize);
            }
        }
    }

    pub fn insert(&mut self, offset: usize, text: &str) {
        self.action(Action::Insert);
        if let Some(ref mut node) = self.pieces.get_node(&Piece::offset(offset)) {
            let added_start = self.added.len();
            self.added.push_str(text);
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

    pub fn replace(&mut self, range: OffsetRange, text: &str) {
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
                    self.insert(loffset, &text[roffset..roffset + len]);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert() {
        let mut pieces = PieceTable::new();
        pieces.insert(0, "the fox jumps over the dog");
        pieces.insert(4, "quick brown ");
        pieces.insert(35, "lazy ");
        pieces.append(" 🐶");
        pieces.insert(0, "🦊 ");
        pieces.insert(56, ", so quick");

        print!("{}", pieces.pieces.dump_tree_as_dot());
        assert_eq!(
            pieces.text(),
            "🦊 the quick brown fox jumps over the lazy dog 🐶, so quick"
        );
    }

    #[test]
    fn delete() {
        let mut pieces = PieceTable::new();
        pieces.insert(0, "the fox jumps over the dog");
        pieces.insert(3, " quick brown");
        pieces.insert(35, "lazy ");
        pieces.append(" 🐶");
        pieces.insert(0, "🦊 ");
        pieces.insert(56, ", so quick");

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
        assert_eq!(
            pieces.text(),
            "🦊 the fox jumps over the dog 🐶, so quick"
        );
    }

    #[test]
    fn replace() {
        let mut pieces = PieceTable::new();
        pieces.insert(0, "the fox jumps over the dog");
        pieces.insert(4, "quick brown ");
        pieces.insert(35, "lazy ");
        pieces.append(" 🐶");
        pieces.insert(0, "🦊 ");
        pieces.insert(56, ", so quick");

        pieces.replace(OffsetRange::new(9, 11), "sneaky"); // "quick brown| "
        pieces.replace(OffsetRange::new(35, 8), "mighty bear"); // "|lazy |dog|"

        print!("{}", pieces.pieces.dump_tree_as_dot());
        assert_eq!(
            pieces.text(),
            "🦊 the sneaky fox jumps over the mighty bear 🐶, so quick"
        );
    }

    #[test]
    fn apply_diff() {
        let mut pieces = PieceTable::new();
        pieces.insert(0, "the fox jumps over the dog");
        pieces.insert(4, "quick brown ");
        pieces.insert(35, "lazy ");
        pieces.append(" 🐶");
        pieces.insert(0, "🦊 ");
        pieces.insert(56, ", so quick");

        let new_text = "🦊 the sneaky fox jumps over the mighty bear 🐶, so quick";
        pieces.apply_diff(new_text);

        print!("{}", pieces.pieces.dump_tree_as_dot());
        assert_eq!(pieces.text(), new_text);
    }

    #[test]
    fn consecutive() {
        let mut pieces = PieceTable::new();
        pieces.append("Where is");
        pieces.append(" ");
        pieces.append("my mind");
        pieces.append("?");
        assert_eq!(pieces.text(), "Where is my mind?");
        assert_eq!(pieces.pieces.len(), 4);

        pieces.pieces.repack();
        assert_eq!(pieces.text(), "Where is my mind?");
        assert_eq!(pieces.pieces.len(), 1);

        pieces.insert(0, "Hey. ");
        pieces.pieces.repack();
        assert_eq!(pieces.text(), "Hey. Where is my mind?");
        assert_eq!(pieces.pieces.len(), 2);
    }

    #[test]
    fn undo_redo() {
        let text = "🦊 the quick brown fox jumps over the lazy dog 🐶, so quick";
        let mut pieces = PieceTable::with_text(text);

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
        pieces.append(", so fast");
        pieces.insert(9, "really ");
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
        pieces.append("first line\nsecond line\nthe end");
        assert_eq!(
            pieces.newlines,
            [10, 22].iter().map(|&i| i as usize).collect()
        );
        pieces.insert(11, "surprise line\n");
        assert_eq!(
            pieces.newlines,
            [10, 24, 36].iter().map(|&i| i as usize).collect()
        );
        pieces.replace(OffsetRange::new(11, 8), "another");
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
        let pieces = PieceTable::with_text("first line\nsecond line\nthe end");
        assert_eq!(pieces.line_length(1), 10);
        assert_eq!(pieces.line_length(2), 11);
        assert_eq!(pieces.line_length(3), 7);
        assert_eq!(pieces.line_length(4), 0);

        assert_eq!(pieces.longest_line(), (2, 11));
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

        let pieces = PieceTable::with_text(text);
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
            pieces.lines_range((7, 10), (8, 9)),
            vec!["quand vous aurez".to_owned(), "Soulagé".to_owned()]
        );

        let coord = (5, 5).into();
        assert_eq!(pieces.offset_to_coord(pieces.coord_to_offset(coord)), coord);

        assert_eq!(
            pieces.offset_to_coord(pieces.coord_to_offset((3, 999).into())),
            Coord { l: 3, c: 1 }
        );

        assert_eq!(pieces.coord_to_offset((9999, 9999).into()), pieces.len());
    }
}
