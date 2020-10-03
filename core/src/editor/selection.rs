#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    pub anchor: usize,
    pub cursor: usize,
}

impl Selection {
    pub fn new() -> Selection {
        Default::default()
    }

    pub fn begin(&self) -> usize {
        if self.anchor <= self.cursor {
            self.anchor
        } else {
            self.cursor
        }
    }

    pub fn end(&self) -> usize {
        if self.anchor < self.cursor {
            self.cursor
        } else {
            self.anchor
        }
    }
}
