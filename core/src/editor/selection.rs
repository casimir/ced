use crate::editor::range::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selection {
    pub anchor: usize,
    pub cursor: usize,
}

impl Default for Selection {
    fn default() -> Selection {
        Selection {
            anchor: 1,
            cursor: 1,
        }
    }
}

impl Range for Selection {
    fn start(&self) -> usize {
        if self.anchor <= self.cursor {
            self.anchor
        } else {
            self.cursor
        }
    }

    fn end(&self) -> usize {
        if self.anchor < self.cursor {
            self.cursor
        } else {
            self.anchor
        }
    }
}

impl Selection {
    pub fn new() -> Selection {
        Default::default()
    }

    pub fn len(&self) -> usize {
        self.overlap(self);
        if self.anchor < self.cursor {
            self.cursor - self.anchor
        } else {
            self.anchor - self.cursor
        }
    }

    pub fn select_left(&mut self, count: usize) {
        if count <= self.cursor {
            self.cursor -= count;
        } else {
            self.cursor = 1;
        }
    }

    pub fn select_right(&mut self, count: usize) {
        self.cursor += count;
    }

    pub fn select_to(&mut self, offset: usize) {
        let to_left = offset < self.cursor;
        self.cursor = offset;
        if to_left {
            self.cursor += 1;
        } else {
            self.cursor -= 1;
        }
    }
}
