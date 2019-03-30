use std::cmp::Ordering;

use crate::datastruct::RBTree;

#[derive(Clone, Copy, Debug, Eq)]
struct Piece {
    offset: usize,
    start: usize,
    length: usize,
    original: bool,
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
        self.offset <= other.offset && other.offset < self.length + self.offset
    }
}

pub struct PieceTable {
    original: String,
    added: String,
    pieces: RBTree<Piece>,
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
        }
    }

    pub fn new_empty() -> PieceTable {
        PieceTable {
            original: String::new(),
            added: String::new(),
            pieces: RBTree::new(),
        }
    }

    pub fn text(&self) -> String {
        self.pieces
            .iter()
            .map(|n| {
                let piece = n.data();
                let buffer = if piece.original {
                    &self.original
                } else {
                    &self.added
                };
                String::from(&buffer[piece.start..(piece.start + piece.length)])
            })
            .collect::<Vec<String>>()
            .join("")
    }

    pub fn lines(&self) -> Vec<String> {
        self.text().lines().map(ToOwned::to_owned).collect()
    }

    pub fn append(&mut self, text: &str) {
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
}
