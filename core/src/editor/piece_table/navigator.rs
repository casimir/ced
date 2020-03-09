use crate::editor::piece_table::{Coords, PieceTable, Position};

const BEGIN: Coords = Coords { l: 1, c: 1 };

pub struct Navigator<'a> {
    pub table: &'a PieceTable,
    cursor: Coords,
}

impl Navigator<'_> {
    pub fn new(table: &PieceTable) -> Navigator<'_> {
        Self::from_position(table, BEGIN)
    }

    pub fn from_position(table: &PieceTable, coords: Coords) -> Navigator<'_> {
        Navigator {
            table,
            cursor: coords,
        }
    }

    pub fn pos(&self) -> Position {
        Position {
            offset: self.table.coord_to_offset(self.cursor),
            coords: self.cursor,
            grapheme: self.table.char_at(self.cursor),
        }
    }

    pub fn next(&mut self) -> &mut Self {
        if self.table.char_at(self.cursor).map_or(false, |c| c == "\n") {
            self.cursor = Coords {
                l: self.cursor.l + 1,
                c: 1,
            }
        } else {
            self.cursor.c += 1;
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
        self
    }

    pub fn line_end(&mut self) -> &mut Self {
        self.cursor.c = self.table.line_length(self.cursor.l);
        self
    }

    pub fn begin(&mut self) -> &mut Self {
        self.cursor = BEGIN;
        self
    }

    pub fn end(&mut self) -> &mut Self {
        self.cursor = self.table.offset_to_coord(self.table.len());
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

        let mut nv = table.navigate(None);
        nv.next();
        assert_eq!(nv.pos().coords, (1, 2).into());
        nv.previous().previous();
        assert_eq!(nv.pos().coords, (1, 1).into());
        nv.previous();
        assert_eq!(nv.pos().coords, (1, 1).into());

        let mut nv = table.navigate(Coords { l: 3, c: 26 });
        nv.next();
        assert_eq!(nv.pos().coords, (3, 27).into());

        let mut nv = table.navigate(Coords { l: 2, c: 1 });
        nv.previous();
        assert_eq!(nv.pos().coords, (1, 16).into());
        nv.next();
        assert_eq!(nv.pos().coords, (2, 1).into());
    }

    #[test]
    fn line() {
        let table = make_table();
        let mut nv = table.navigate(None);

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
        let mut nv = table.navigate(None);
        nv.end();
        assert_eq!(nv.pos().coords, (3, 27).into());
        nv.begin();
        assert_eq!(nv.pos().coords, (1, 1).into());
    }
}
