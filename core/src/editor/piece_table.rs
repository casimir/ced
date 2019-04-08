use std::cmp::Ordering;

use crate::datastruct::{Consecutive, RBNode, RBTree};
use crate::editor::diff::{diff, Diff};

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

    fn range(self, offset: usize, length: usize) -> Piece {
        let end = offset + length;
        if self.contains(offset) && self.contains(end) {
            Piece {
                offset,
                start: self.start + offset - self.offset,
                length,
                ..self
            }
        } else if self.contains(offset) {
            Piece {
                offset,
                start: self.start + offset - self.offset,
                length: self.length - (offset - self.offset),
                ..self
            }
        } else if self.contains(end) {
            Piece {
                length: end - self.offset,
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

pub struct PieceTable {
    original: String,
    added: String,
    pieces: RBTree<Piece>,
    last_action: Option<Action>,
    undos: Vec<RBTree<Piece>>,
    redos: Vec<RBTree<Piece>>,
}

impl PieceTable {
    pub fn new(text: &str) -> PieceTable {
        let mut pieces = RBTree::new();
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
            last_action: None,
            undos: Vec::new(),
            redos: Vec::new(),
        }
    }

    pub fn new_empty() -> PieceTable {
        PieceTable {
            original: String::new(),
            added: String::new(),
            pieces: RBTree::new(),
            last_action: None,
            undos: Vec::new(),
            redos: Vec::new(),
        }
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

    pub fn text_range(&self, offset: usize, length: usize) -> Option<String> {
        let end = offset + length;
        self.pieces.get(&Piece::offset(offset)).map(|start_piece| {
            start_piece
                .values()
                .take_while(|p| p.offset < end)
                .map(|p| {
                    let buffer = if p.original {
                        &self.original
                    } else {
                        &self.added
                    };
                    let ranged = p.range(offset, length);
                    String::from(&buffer[ranged.start..ranged.end()])
                })
                .collect::<Vec<String>>()
                .join("")
        })
    }

    pub fn lines(&self) -> Vec<String> {
        self.text().lines().map(ToOwned::to_owned).collect()
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
    }

    fn shift_offset_after(&mut self, node: &RBNode<Piece>, value: i64) {
        if value != 0 {
            for n in node.iter().skip(1) {
                n.apply(|p| p.offset = (p.offset as i64 + value) as usize);
            }
        }
    }

    pub fn insert(&mut self, offset: usize, text: &str) {
        self.action(Action::Insert);
        if let Some(ref mut node) = self.pieces.get(&Piece::offset(offset)) {
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
            self.pieces.delete_node(node);
            if let Some(p) = sub_pieces.0 {
                self.pieces.insert(p);
            }
            self.pieces.insert(new);
            self.pieces.insert(sub_pieces.1);
        } else {
            self.append(text);
        }
    }

    pub fn delete(&mut self, offset: usize, length: usize) {
        self.action(Action::Delete);
        if let Some(start_node) = self.pieces.get(&Piece::offset(offset)) {
            let pieces = start_node
                .values()
                .take_while(|p| p.offset < offset + length)
                .collect::<Vec<Piece>>();
            let (head, _) = pieces[0].truncate(offset, length);
            let (_, tail) = pieces[pieces.len() - 1].truncate(offset, length);
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
                self.shift_offset_after(&ln, -(length as i64));
            } else {
                start_node.apply(|n| n.offset -= length);
                self.shift_offset_after(&start_node, -(length as i64));
            }
        }
    }

    pub fn replace(&mut self, start: usize, length: usize, text: &str) {
        self.start_bulk();
        self.delete(start, length);
        self.insert(start, text);
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
                Diff::Left(len) => self.delete(loffset, len),
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
        let mut pieces = PieceTable::new_empty();
        pieces.insert(0, "the fox jumps over the dog");
        pieces.insert(4, "quick brown ");
        pieces.insert(35, "lazy ");
        pieces.append(" 🐶");
        pieces.insert(0, "🦊 ");
        pieces.insert(56, ", so quick");

        print!("{}", pieces.pieces.dump_as_dot());
        assert_eq!(
            pieces.text(),
            "🦊 the quick brown fox jumps over the lazy dog 🐶, so quick"
        );
    }

    #[test]
    fn delete() {
        let mut pieces = PieceTable::new_empty();
        pieces.insert(0, "the fox jumps over the dog");
        pieces.insert(3, " quick brown");
        pieces.insert(35, "lazy ");
        pieces.append(" 🐶");
        pieces.insert(0, "🦊 ");
        pieces.insert(56, ", so quick");

        pieces.delete(9, 12); // "quick brown| "
        pieces.delete(28, 5); // "|lazy |"

        print!("{}", pieces.pieces.dump_as_dot());
        assert_eq!(
            pieces.text(),
            "🦊 the fox jumps over the dog 🐶, so quick"
        );
    }

    #[test]
    fn replace() {
        let mut pieces = PieceTable::new_empty();
        pieces.insert(0, "the fox jumps over the dog");
        pieces.insert(4, "quick brown ");
        pieces.insert(35, "lazy ");
        pieces.append(" 🐶");
        pieces.insert(0, "🦊 ");
        pieces.insert(56, ", so quick");

        pieces.replace(9, 11, "sneaky"); // "quick brown| "
        pieces.replace(35, 8, "mighty bear"); // "|lazy |dog|"

        print!("{}", pieces.pieces.dump_as_dot());
        assert_eq!(
            pieces.text(),
            "🦊 the sneaky fox jumps over the mighty bear 🐶, so quick"
        );
    }

    #[test]
    fn apply_diff() {
        let mut pieces = PieceTable::new_empty();
        pieces.insert(0, "the fox jumps over the dog");
        pieces.insert(4, "quick brown ");
        pieces.insert(35, "lazy ");
        pieces.append(" 🐶");
        pieces.insert(0, "🦊 ");
        pieces.insert(56, ", so quick");

        let new_text = "🦊 the sneaky fox jumps over the mighty bear 🐶, so quick";
        pieces.apply_diff(new_text);

        print!("{}", pieces.pieces.dump_as_dot());
        assert_eq!(pieces.text(), new_text);
    }

    #[test]
    fn consecutive() {
        let mut pieces = PieceTable::new_empty();
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
        let mut pieces = PieceTable::new(text);

        // empty undo stack
        assert!(!pieces.undo());

        // delete + undo/redo
        pieces.delete(53, 10);
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
        pieces.delete(0, 5);
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
}
