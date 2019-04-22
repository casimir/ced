#[derive(Clone, Copy, Debug)]
pub struct Range {
    offset: usize,
    length: usize,
}

impl Range {
    pub fn new(offset: usize, length: usize) -> Range {
        Range { offset, length }
    }

    #[inline]
    pub fn start(&self) -> usize {
        self.offset
    }

    #[inline]
    pub fn end(&self) -> usize {
        self.offset + self.length
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }

    #[inline]
    pub fn overlap(&self, other: &Range) -> bool {
        (self.start() <= other.start() && other.start() < self.end())
            || (other.start() <= self.start() && self.start() < other.end())
    }

    #[inline]
    pub fn contains(&self, other: &Range) -> bool {
        self.start() <= other.start() && other.end() <= self.end()
    }
}
