#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selection {
    pub anchor: usize,
    pub cursor: usize,
    pub(crate) target_col: usize,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            anchor: Default::default(),
            cursor: Default::default(),
            target_col: 1,
        }
    }
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
