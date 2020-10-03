use crate::editor::piece_table::{Coords, Navigator, PieceTable};

#[derive(Debug)]
pub struct Position {
    pub offset: usize,
    pub coords: Coords,
    pub grapheme: Option<String>,
}

pub struct PositionIterator<'a> {
    table: &'a PieceTable,
    nv: Navigator<'a>,
    offset: usize,
    started: bool,
}

impl PositionIterator<'_> {
    fn pos(&self) -> Position {
        Position {
            offset: self.offset,
            coords: self.table.offset_to_coord(self.offset).unwrap(),
            grapheme: self.table.char_at_offset(self.offset),
        }
    }
}

impl<'a> Iterator for PositionIterator<'a> {
    type Item = Position;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.started {
            self.started = true;
            return Some(self.pos());
        }

        self.nv.next();
        if self.nv.is_at_end() {
            Some(self.nv.pos())
        } else {
            None
        }
    }
}

impl<'a> From<&'a PieceTable> for PositionIterator<'a> {
    fn from(table: &'a PieceTable) -> PositionIterator<'a> {
        PositionIterator {
            table,
            nv: table.navigate(None).unwrap(),
            offset: 0,
            started: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::ByteSlice;

    #[test]
    fn positions() {
        let table = PieceTable::with_text("line 1\n2 line".to_owned());
        let positions = vec![
            (1, 1),
            (1, 2),
            (1, 3),
            (1, 4),
            (1, 5),
            (1, 6),
            (1, 7),
            (2, 1),
            (2, 2),
            (2, 3),
            (2, 4),
            (2, 5),
            (2, 6),
        ];
        for (i, p) in PositionIterator::from(&table).enumerate() {
            assert_eq!(p.coords, Coords::from(positions[i]));
        }
    }

    #[test]
    fn ascii() {
        let text = "line 1\n2 line";
        let chars: Vec<char> = text.chars().collect();
        let table = PieceTable::with_text(text.to_owned());
        for (i, p) in PositionIterator::from(&table).enumerate() {
            if i < chars.len() {
                assert_eq!(p.grapheme, Some(chars[i].to_string()));
            } else {
                assert_eq!(p.grapheme, None);
            }
            assert_eq!(p.offset, i);
        }
    }

    #[test]
    fn unicode() {
        let text = "🚀∂⏰ØⓏ 1️⃣\n2 line";
        let chars: Vec<(usize, &str)> = text
            .as_bytes()
            .grapheme_indices()
            .map(|(i, _, g)| (i, g))
            .collect();
        let table = PieceTable::with_text(text.to_owned());
        for (i, p) in PositionIterator::from(&table).enumerate() {
            if i < chars.len() {
                assert_eq!(p.grapheme, Some(chars[i].1.to_string()));
                assert_eq!(p.offset, chars[i].0);
            } else {
                assert_eq!(p.grapheme, None);
                assert_eq!(p.offset, text.len());
            }
        }
    }
}
