use std::cmp::min;

use bstr::ByteSlice;

use crate::editor::piece_table::{Coords, PieceTable, Position};

const BEGIN: Coords = Coords { l: 1, c: 1 };

pub struct Navigator<'a> {
    pub table: &'a PieceTable,
    cursor: Coords,
    pub(crate) target_col: usize,
}

impl Navigator<'_> {
    pub fn new(table: &PieceTable) -> Navigator<'_> {
        Self::from_position(table, BEGIN).unwrap()
    }

    pub fn from_position(table: &PieceTable, coords: Coords) -> Option<Navigator<'_>> {
        if table.coord_to_offset(coords).is_some() {
            Some(Navigator {
                table,
                cursor: coords,
                target_col: coords.c,
            })
        } else {
            None
        }
    }

    pub fn is_at_end(&self) -> bool {
        self.cursor == self.table.max_coord()
    }

    pub fn pos(&self) -> Position {
        let col_count = self
            .table
            .line_bytes(self.cursor.l)
            .unwrap_or_else(|| panic!("get bytes for line {}", self.cursor.l))
            .graphemes()
            .count()
            + 1;
        let cursor = Coords {
            c: min(col_count, self.target_col),
            ..self.cursor
        };
        Position {
            offset: self
                .table
                .coord_to_offset(cursor)
                .unwrap_or_else(|| panic!("convert coordinates: {:?}", cursor)),
            coords: cursor,
            grapheme: self.table.char_at(cursor),
        }
    }

    pub fn next(&mut self) -> &mut Self {
        if self.cursor == self.table.max_coord() {
            return self;
        } else if self.table.char_at(self.cursor).map_or(false, |c| c == "\n") {
            self.cursor = Coords {
                l: self.cursor.l + 1,
                c: 1,
            };
            self.target_col = 1;
        } else {
            let offset = self.table.coord_to_offset(self.cursor).unwrap();
            if let Some(coord) = self.table.offset_to_coord(offset + 1) {
                self.cursor = coord;
                self.target_col = coord.c;
            }
        }
        self
    }

    pub fn previous(&mut self) -> &mut Self {
        if self.cursor != BEGIN {
            if self.cursor.l != 1 && self.cursor.c == 1 {
                let l = self.cursor.l - 1;
                self.cursor.l = l;
                self.cursor.c = self.table.line_length(l) + 1;
            } else {
                self.cursor.c -= 1;
            }
            self.target_col = self.cursor.c;
        }
        self
    }

    pub fn next_line(&mut self) -> &mut Self {
        if self.cursor.l < self.table.line_count() {
            self.cursor.l += 1;
        }
        self
    }

    pub fn previous_line(&mut self) -> &mut Self {
        if self.cursor.l > 1 {
            self.cursor.l -= 1;
        }
        self
    }

    pub fn line_begin(&mut self) -> &mut Self {
        self.cursor.c = 1;
        self.target_col = self.cursor.c;
        self
    }

    pub fn line_end(&mut self) -> &mut Self {
        self.cursor.c = self.table.line_length(self.cursor.l);
        self.target_col = self.cursor.c;
        self
    }

    pub fn begin(&mut self) -> &mut Self {
        self.cursor = BEGIN;
        self.target_col = BEGIN.c;
        self
    }

    pub fn end(&mut self) -> &mut Self {
        self.cursor = self.table.max_coord();
        self.target_col = self.cursor.c;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table() -> PieceTable {
        let lines = vec![
            "Nam quis nulla.",                                           // 1 - 15
            "Integer malesuada. In in enim a arcu imperdiet malesuada.", // 2 - 57
            "Sed vel lectus. Donec odio",                                // 3 - 26
        ];
        PieceTable::with_text(
            lines
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join("\n"),
        )
    }

    #[test]
    fn char() {
        let table = make_table();

        let mut nv = table.navigate(None).unwrap();
        nv.next();
        assert_eq!(nv.pos().coords, (1, 2).into());
        nv.previous().previous();
        assert_eq!(nv.pos().coords, (1, 1).into());
        nv.previous();
        assert_eq!(nv.pos().coords, (1, 1).into());

        let mut nv = table.navigate(Coords { l: 3, c: 26 }).unwrap();
        nv.next();
        assert_eq!(nv.pos().coords, (3, 26).into());

        let mut nv = table.navigate(Coords { l: 2, c: 1 }).unwrap();
        nv.previous();
        assert_eq!(nv.pos().coords, (1, 16).into());
        nv.next();
        assert_eq!(nv.pos().coords, (2, 1).into());
    }

    #[test]
    fn line() {
        let table = make_table();
        let mut nv = table.navigate(None).unwrap();

        nv.next_line();
        assert_eq!(nv.pos().coords, (2, 1).into());
        nv.next_line();
        assert_eq!(nv.pos().coords, (3, 1).into());
        nv.next_line();
        assert_eq!(nv.pos().coords, (3, 1).into());

        nv.previous_line().previous_line();
        assert_eq!(nv.pos().coords, (1, 1).into());
        nv.previous_line();
        assert_eq!(nv.pos().coords, (1, 1).into());
    }

    #[test]
    fn whole() {
        let table = make_table();
        let mut nv = table.navigate(None).unwrap();
        nv.end();
        assert_eq!(nv.pos().coords, (3, 26).into());
        assert!(nv.is_at_end());
        nv.begin();
        assert_eq!(nv.pos().coords, (1, 1).into());
    }
}
