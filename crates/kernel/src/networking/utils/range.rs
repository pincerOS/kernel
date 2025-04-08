#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    start: i32,
    end: i32,
}

impl Range {
    pub const fn new(start: i32, end: i32) -> Self {
        Self { start, end }
    }

    pub fn slice<'a, T>(&self, data: &'a [T]) -> &'a [T] {
        &data[self.start..self.end]
    }
}

impl Iterator for Range {
    type Item = i32;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.start < self.end {
            let current = self.start;
            self.start += 1;
            Some(current)
        } else {
            None
        }
    }
}
