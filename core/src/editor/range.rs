// TODO investigate how to reuse RangeBounds from std instead
pub trait Range {
    fn start(&self) -> usize;
    fn end(&self) -> usize;

    fn len(&self) -> usize {
        self.end() - self.start()
    }

    fn overlap(&self, other: &dyn Range) -> bool {
        (self.start() <= other.start() && other.start() < self.end())
            || (other.start() <= self.start() && self.start() < other.end())
    }

    fn contains(&self, other: &dyn Range) -> bool {
        self.start() <= other.start() && other.end() <= self.end()
    }
}

impl Range for std::ops::Range<usize> {
    fn start(&self) -> usize {
        self.start
    }

    fn end(&self) -> usize {
        self.end
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OffsetRange {
    offset: usize,
    length: usize,
}

impl OffsetRange {
    pub fn new(offset: usize, length: usize) -> OffsetRange {
        OffsetRange { offset, length }
    }
}

impl Range for OffsetRange {
    fn start(&self) -> usize {
        self.offset
    }

    fn end(&self) -> usize {
        self.offset + self.length
    }

    fn len(&self) -> usize {
        self.length
    }
}
