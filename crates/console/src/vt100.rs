use alloc::boxed::Box;
use alloc::vec;

use crate::color::rgba;
use crate::grid::{Colors, GridRef};

// TODO: line/row separation
// TODO: handling double width chars (emoji)
pub struct EmulatorState {
    pub rows: usize,
    pub cols: usize,
    pub scrolled_rows: usize,
    stride: usize,
    chars: Box<[char]>,
    colors: Box<[Colors]>,
    changed: bool,

    pub cursor: GridCoords,
    cur_mode: Colors,
}

#[derive(Copy, Clone)]
pub struct GridCoords {
    pub row: usize,
    pub col: usize,
}

const DEFAULT_COLOR: Colors = Colors {
    fg: rgba(255, 255, 255, 255),
    bg: rgba(0, 0, 0, 0),
};

impl EmulatorState {
    pub fn new(rows: usize, cols: usize) -> Self {
        let stride = cols.next_multiple_of(4);
        Self {
            rows,
            cols,
            scrolled_rows: 0,
            stride,
            chars: vec![' '; rows * stride].into_boxed_slice(),
            colors: vec![DEFAULT_COLOR; rows * stride].into_boxed_slice(),
            changed: true,
            cursor: GridCoords { row: 0, col: 0 },
            cur_mode: DEFAULT_COLOR,
        }
    }

    fn scroll(&mut self, distance: usize) {
        if distance == 0 {
            return;
        }
        for row in 0..self.rows.saturating_sub(distance) {
            let dst_start = row * self.stride;
            let dst_end = dst_start + self.stride;
            let src_start = (row + distance) * self.stride - dst_end;
            let src_end = src_start + self.stride;

            let (dst, src) = self.chars.split_at_mut(dst_end);
            dst[dst_start..dst_end].copy_from_slice(&src[src_start..src_end]);
            let (dst, src) = self.colors.split_at_mut(dst_end);
            dst[dst_start..dst_end].copy_from_slice(&src[src_start..src_end]);
        }

        let clear_start = self.rows.saturating_sub(distance);
        let clear_end = self.rows;
        let clear_idx_range = clear_start * self.stride..clear_end * self.stride;

        let fill_color = DEFAULT_COLOR;
        self.chars[clear_idx_range.clone()].fill(' ');
        self.colors[clear_idx_range].fill(fill_color);

        self.cursor.row = self.cursor.row.saturating_sub(distance);
        self.scrolled_rows += distance;
    }

    pub fn should_wrap(&self) -> bool {
        self.cursor.col >= self.cols
    }
    pub fn check_wrap(&mut self) {
        if self.should_wrap() {
            self.wrap();
        }
    }
    pub fn wrap(&mut self) {
        self.cursor.col = 0;
        self.cursor.row += 1;
        if self.cursor.row == self.rows {
            self.scroll(1);
        }
    }

    pub fn set_char_color(&mut self, pos: GridCoords, char: char, mode: Colors) {
        if pos.row < self.rows && pos.col < self.cols {
            self.chars[pos.row * self.stride + pos.col] = char;
            self.colors[pos.row * self.stride + pos.col] = mode;
        }
    }

    pub fn input(&mut self, text: &[u8]) {
        for chunk in text.utf8_chunks() {
            for c in chunk.valid().chars() {
                match c {
                    '\r' => self.cursor.col = 0,
                    '\n' => self.wrap(),
                    '\t' => {
                        self.cursor.col = (self.cursor.col + 1).next_multiple_of(8);
                        self.check_wrap();
                    }
                    _ => {
                        self.set_char_color(self.cursor, c, self.cur_mode);
                        self.cursor.col += 1;
                        self.check_wrap();
                    }
                }
            }
            for _ in chunk.invalid() {
                self.set_char_color(self.cursor, ' ', self.cur_mode);
                self.cursor.col += 1;
                self.check_wrap();
            }
        }
        self.changed = true;
    }

    pub fn update(&mut self, grid: GridRef<'_>) {
        if self.changed {
            for row in 0..self.rows.min(grid.rows) {
                for col in 0..self.cols.min(grid.cols) {
                    let src = row * self.stride + col;
                    let dst = row * grid.stride + col;
                    grid.chars[dst] = self.chars[src];
                    grid.colors[dst] = self.colors[src];
                }
            }
        }
    }
}

// #[test]
// fn term_test() {
//     let width = 1280;
//     let height = 720;
//     let char_dims = (16, 8);

//     let scale = 2;
//     let hpad = 2;
//     let vpad = 0;
//     let rows = (height - 2 * vpad) / (char_dims.0 * scale);
//     let cols = (width - 2 * hpad) / (char_dims.1 * scale);

//     let mut text_buf = crate::TextBuf::new();

//     let mut emulator = EmulatorState::new(rows - 2, cols - 2);
//     // let mut log = LogGenerator::new();

//     loop {
//         if let Some(text) = log.next(&mut text_buf) {
//             emulator.input(text);
//         }

//         // emulator.update(&mut grid, rows - 2, cols - 2, cols + 1, cols);
//     }
// }
