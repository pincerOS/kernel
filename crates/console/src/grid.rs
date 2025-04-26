use alloc::boxed::Box;
use alloc::vec;

pub struct CharGrid {
    pub chars: Box<[char]>,
    pub colors: Box<[Colors]>,
    pub rows: usize,
    pub cols: usize,
    pub stride: usize,
    pub char_dims: (usize, usize),
    pub scale: usize,
    pub hpad: usize,
    pub vpad: usize,
}

#[derive(Copy, Clone)]
pub struct Colors {
    pub fg: u32,
    pub bg: u32,
}

pub struct GridRef<'a> {
    pub chars: &'a mut [char],
    pub colors: &'a mut [Colors],
    pub cols: usize,
    pub rows: usize,
    pub stride: usize,
}

impl CharGrid {
    pub fn region(&mut self, r: usize, c: usize, rows: usize, cols: usize) -> GridRef<'_> {
        let base = r * self.stride + c;
        GridRef {
            chars: &mut self.chars[base..],
            colors: &mut self.colors[base..],
            cols,
            rows,
            stride: self.stride,
        }
    }

    pub fn new(
        (width, height): (usize, usize),
        char_dims: (usize, usize),
        scale: usize,
        hpad: usize,
        vpad: usize,
        fill_color: Colors,
    ) -> Self {
        let rows = (height - 2 * vpad) / (char_dims.0 * scale);
        let cols = (width - 2 * hpad) / (char_dims.1 * scale);
        let stride = cols;
        Self {
            chars: vec![' '; rows * stride].into_boxed_slice(),
            colors: vec![fill_color; rows * stride].into_boxed_slice(),
            rows,
            cols,
            stride,
            char_dims,
            scale,
            hpad,
            vpad,
        }
    }
}
