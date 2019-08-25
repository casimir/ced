// TODO investigate how to reuse RangeBounds from std instead
pub trait Range {
    fn start(&self) -> usize;
    fn end(&self) -> usize;

    fn overlap(&self, other: &Range) -> bool {
        (self.start() <= other.start() && other.start() < self.end())
            || (other.start() <= self.start() && self.start() < other.end())
    }

    fn contains(&self, other: &Range) -> bool {
        self.start() <= other.start() && other.end() <= self.end()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OffsetRange {
    offset: usize,
    length: usize,
}

impl Range for OffsetRange {
    fn start(&self) -> usize {
        self.offset
    }

    fn end(&self) -> usize {
        self.offset + self.length
    }
}

impl OffsetRange {
    pub fn new(offset: usize, length: usize) -> OffsetRange {
        OffsetRange { offset, length }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }
}
