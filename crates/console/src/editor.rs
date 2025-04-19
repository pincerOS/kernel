// See also:
// - [Text Rendering Hates You](https://faultlore.com/blah/text-hates-you/)
// - [Text Editing Hates You Too](https://lord.io/text-editing-hates-you-too/)

use core::ops::ControlFlow;

use crate::color::rgba;
use crate::grid::{Colors, GridRef};
use crate::vt100::{EmulatorState, GridCoords};

#[derive(Debug)]
pub enum Keypress {
    Char(Modifiers, char, char),
    Function(Modifiers, FuncKey),
    Invalid([u8; 8]),
}

#[derive(Debug)]
pub enum KeyEvent {
    Press(Keypress),
    Repeat(Keypress),
    Release(Keypress),
}

impl KeyEvent {
    pub fn new(event: KeyEventMode, key: Keypress) -> Self {
        match event {
            KeyEventMode::Press => Self::Press(key),
            KeyEventMode::Repeat => Self::Repeat(key),
            KeyEventMode::Release => Self::Release(key),
        }
    }
}

pub enum KeyEventMode {
    Press,
    Repeat,
    Release,
}

#[derive(PartialEq, Copy, Clone)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
    };
    pub const CTRL: Modifiers = Modifiers {
        ctrl: true,
        shift: false,
        alt: false,
        meta: false,
    };
    pub const SHIFT: Modifiers = Modifiers {
        ctrl: false,
        shift: true,
        alt: false,
        meta: false,
    };
}

impl core::fmt::Debug for Modifiers {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Modifiers(")?;
        let mut first = true;
        if self.ctrl {
            first = false;
            write!(f, "CTRL")?;
        }
        if self.shift {
            if !first {
                write!(f, " | ")?;
            }
            first = false;
            write!(f, "SHIFT")?;
        }
        if self.alt {
            if !first {
                write!(f, " | ")?;
            }
            first = false;
            write!(f, "ALT")?;
        }
        if self.meta {
            if !first {
                write!(f, " | ")?;
            }
            first = false;
            write!(f, "META")?;
        }
        if first {
            write!(f, "NONE")?;
        }
        write!(f, ")")?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum FuncKey {
    Up,
    Down,
    Left,
    Right,

    Backspace,
    Delete,
    Enter,
    Tab,

    Insert,
    End,
    Home,
    Escape,

    PageUp,
    PageDown,

    Func(u8),
}

pub fn find_char_after(input: &[u8], cursor: usize) -> usize {
    for i in cursor + 1..input.len() {
        if !is_utf8_trailer(input[i]) {
            return i;
        }
    }
    input.len()
}

pub fn find_char_before(input: &[u8], cursor: usize) -> usize {
    for i in (0..cursor).rev() {
        if !is_utf8_trailer(input[i]) {
            return i;
        }
    }
    0
}

#[allow(dead_code)]
fn utf8_width(b: u8) -> Option<usize> {
    match b {
        _ if b & 0b10000000 == 0b00000000 => Some(1),
        _ if b & 0b11100000 == 0b11000000 => Some(2),
        _ if b & 0b11110000 == 0b11100000 => Some(3),
        _ if b & 0b11110000 == 0b11110000 => Some(4),
        _ => None,
    }
}
fn is_utf8_trailer(b: u8) -> bool {
    b & 0b11000000 == 0b10000000
}

#[derive(Clone, Copy)]
pub struct Cursor {
    pub byte: usize,
}

extern crate alloc;

pub struct LineEditor {
    pub buf: alloc::string::String,
    pub cut_buffer: alloc::string::String,
    pub primary: Cursor,
    pub secondary: Cursor,
    pub last_keypress: u64,
}

impl LineEditor {
    pub fn new() -> LineEditor {
        LineEditor {
            buf: alloc::string::String::with_capacity(256),
            cut_buffer: alloc::string::String::with_capacity(256),
            primary: Cursor { byte: 0 },
            secondary: Cursor { byte: 0 },
            last_keypress: 0,
        }
    }

    pub fn input(&mut self, text: &str) {
        let range = self.selection_range();
        self.buf.replace_range(range.clone(), text);

        // TODO: text cols, rows
        let new_cursor = range.start + text.len();
        let new_cursor = Cursor { byte: new_cursor };
        self.primary = new_cursor;
        self.secondary = self.primary;
    }

    pub fn paste_from_cut(&mut self) {
        let range = self.selection_range();
        let text = &self.cut_buffer;
        self.buf.replace_range(range.clone(), text);

        // TODO: text cols, rows
        let new_cursor = range.start + text.len();
        let new_cursor = Cursor { byte: new_cursor };
        self.primary = new_cursor;
        self.secondary = self.primary;
    }

    pub fn range_width(&self, range: core::ops::Range<usize>) -> usize {
        self.buf[range].chars().count()
        // unicode_width_16::UnicodeWidthStr::width(&self.buf[range])
    }

    pub fn selection_range(&self) -> core::ops::Range<usize> {
        if self.primary.byte <= self.secondary.byte {
            self.primary.byte..self.secondary.byte
        } else {
            self.secondary.byte..self.primary.byte
        }
    }

    pub fn cursor_left(&mut self, only_primary: bool) -> Cursor {
        let cursor = self.primary.byte;
        let mut cur = unicode_segmentation::GraphemeCursor::new(cursor, self.buf.len(), false);
        let mut end = cursor;
        if cursor <= self.buf.len() {
            if let Ok(b) = cur.prev_boundary(&self.buf, 0) {
                if let Some(b) = b {
                    end = b;
                } else {
                    end = cursor;
                }
            } else {
                let next_char = find_char_before(self.buf.as_bytes(), cursor);
                end = next_char;
            }
        }
        if only_primary {
            self.primary = Cursor { byte: end };
        } else {
            self.primary = Cursor { byte: end };
            self.secondary = Cursor { byte: end };
        }
        self.primary
    }

    pub fn cursor_right(&mut self, only_primary: bool) -> Cursor {
        let cursor = self.primary.byte;
        let mut cur = unicode_segmentation::GraphemeCursor::new(cursor, self.buf.len(), false);
        let mut end = cursor + 1;
        if cursor <= self.buf.len() {
            if let Ok(b) = cur.next_boundary(&self.buf, 0) {
                if let Some(b) = b {
                    end = b;
                } else {
                    end = cursor;
                }
            } else {
                let next_char = find_char_after(self.buf.as_bytes(), cursor);
                end = next_char;
            }
        }
        end = usize::min(end, self.buf.len());
        if only_primary {
            self.primary = Cursor { byte: end };
        } else {
            self.primary = Cursor { byte: end };
            self.secondary = Cursor { byte: end };
        }
        self.primary
    }

    pub fn delete_left(&mut self) {
        let cur = self.primary;
        if self.primary.byte != self.secondary.byte {
            self.delete_range(self.selection_range());
        } else {
            let moved = self.cursor_left(false);
            if cur.byte < moved.byte {
                panic!("{:?} {:?}", cur.byte, moved.byte)
            }
            self.delete_range(moved.byte..cur.byte);
        }
    }

    pub fn delete_right(&mut self) {
        let cur = self.primary;
        if self.primary.byte != self.secondary.byte {
            self.delete_range(self.selection_range());
        } else {
            let moved = self.cursor_right(false);
            self.delete_range(cur.byte..moved.byte);
        }
    }

    pub fn delete_range(&mut self, range: core::ops::Range<usize>) {
        self.buf.replace_range(range.clone(), "");
        let primary = self.primary;
        let secondary = self.secondary;
        let (left, right) = if primary.byte <= secondary.byte {
            (primary.byte, secondary.byte)
        } else {
            (secondary.byte, primary.byte)
        };

        let range_len = range.end - range.start;
        let remap_idx = |idx| {
            if idx > range.start && idx <= range.end {
                range.start
            } else if idx > range.end {
                idx - range_len
            } else {
                idx
            }
        };

        let new_left = remap_idx(left);
        let new_right = remap_idx(right);

        if left == primary.byte {
            self.primary = Cursor { byte: new_left };
            self.secondary = Cursor { byte: new_right };
        } else {
            self.primary = Cursor { byte: new_right };
            self.secondary = Cursor { byte: new_left };
        }
    }

    pub fn set_cursor(&mut self, byte: usize) {
        self.primary = Cursor { byte };
        self.secondary = self.primary;
    }

    pub fn clear(&mut self) {
        self.set_cursor(0);
        self.buf.clear();
    }

    pub fn set_cursor_range(&mut self, start: usize, end: usize) {
        self.primary = Cursor { byte: end };
        self.secondary = Cursor { byte: start };
    }
}

pub fn editor_input(editor: &mut LineEditor, key: KeyEvent, time_us: u64) -> ControlFlow<()> {
    let c = match key {
        KeyEvent::Press(c) => c,
        KeyEvent::Repeat(c) => c,
        KeyEvent::Release(_) => return ControlFlow::Continue(()),
    };
    match c {
        Keypress::Function(Modifiers::NONE, FuncKey::Right) => {
            editor.cursor_right(false);
        }
        Keypress::Function(Modifiers::NONE, FuncKey::Left) => {
            editor.cursor_left(false);
        }
        Keypress::Function(Modifiers::SHIFT, FuncKey::Right) => {
            editor.cursor_right(true);
        }
        Keypress::Function(Modifiers::SHIFT, FuncKey::Left) => {
            editor.cursor_left(true);
        }
        Keypress::Function(Modifiers::NONE, FuncKey::Delete) => {
            editor.delete_right();
        }
        Keypress::Function(Modifiers::NONE, FuncKey::Backspace) => {
            editor.delete_left();
        }
        Keypress::Function(Modifiers::NONE, FuncKey::End)
        | Keypress::Char(Modifiers::CTRL, 'E' | 'e', _) => {
            editor.set_cursor(editor.buf.len());
        }
        Keypress::Function(Modifiers::NONE, FuncKey::Home)
        | Keypress::Char(Modifiers::CTRL, 'A' | 'a', _) => {
            editor.set_cursor(0);
        }
        Keypress::Char(Modifiers::CTRL, 'K' | 'k', _) => {
            let mut range = editor.selection_range();
            if range.is_empty() {
                range = range.start..editor.buf.len();
            }
            let text = &editor.buf[range.clone()];
            editor.cut_buffer.replace_range(.., text);
            editor.delete_range(range);
        }
        Keypress::Char(Modifiers::CTRL, 'Y' | 'y', _) => {
            editor.paste_from_cut();
        }
        Keypress::Char(Modifiers::CTRL, 'C' | 'c', _) => {
            return ControlFlow::Break(());
        }
        Keypress::Function(Modifiers::NONE, FuncKey::Enter) => {
            editor.input("\n");
        }
        Keypress::Function(Modifiers::NONE, FuncKey::Tab) => {
            // tab_complete();
        }
        Keypress::Char(Modifiers::CTRL, 'D' | 'd', _) => {
            if editor.buf.is_empty() {
                return ControlFlow::Break(());
            }
        }
        Keypress::Char(
            Modifiers {
                ctrl: false,
                alt: false,
                meta: false,
                shift: _,
            },
            _,
            c,
        ) => {
            let mut buf = [0u8; 8];
            let str = c.encode_utf8(&mut buf);
            editor.input(str);
        }
        _c => {
            // eprintln!("{:?}", c);
        }
    }
    editor.last_keypress = time_us;

    ControlFlow::Continue(())
}

pub fn draw_editor(editor: &LineEditor, grid: GridRef<'_>, color: u32, blink: bool) -> usize {
    for r in 0..grid.rows {
        for c in 0..grid.cols {
            grid.chars[r * grid.stride + c] = ' ';
            grid.colors[r * grid.stride + c] = Colors {
                fg: color,
                bg: rgba(0, 0, 0, 0),
            };
        }
    }

    let wrap_at = grid.cols;
    let selection = editor.selection_range();
    let primary_pos = editor.primary.byte;

    let cursor_width = 1;
    let line_height = 1;

    let mut x = 0;
    let mut y = 0;
    for (i, char) in editor.buf.char_indices() {
        if y >= grid.rows {
            break;
        }
        match char {
            '\r' => continue,
            '\n' => {
                if i == primary_pos {
                    // TODO: slightly less weird behavior here (full line, cursor at end)
                    // Maybe make it always wrap a blank line if there isn't width for the cursor?
                    if x + cursor_width > wrap_at {
                        x = 0;
                        y += line_height;
                    }
                    if y >= grid.rows {
                        break;
                    }
                    if blink {
                        let bg_color = rgba(255, 255, 255, 255);
                        // TODO: proper background fill
                        grid.colors[grid.stride * y + x] = Colors {
                            fg: 0,
                            bg: bg_color,
                        };
                        grid.chars[grid.stride * y + x] = ' ';
                    }
                }

                x = 0;
                y += line_height;
                continue;
            }
            _ => (),
        }

        let glyph_width = 1;
        if x + glyph_width > wrap_at {
            x = 0;
            y += line_height;
        }
        if y >= grid.rows {
            break;
        }

        let mut colors = Colors {
            fg: color,
            bg: rgba(0, 0, 0, 0),
        };
        if selection.contains(&i) || (i == primary_pos && blink) {
            colors.fg = rgba(0, 0, 0, 255);
            colors.bg = rgba(255, 255, 255, 255);
        }

        grid.colors[grid.stride * y + x] = colors;
        grid.chars[grid.stride * y + x] = char;
        x += glyph_width;
    }

    if blink && editor.buf.len() == primary_pos {
        if x + cursor_width > wrap_at {
            x = 0;
            y += line_height;
        }
        if y < grid.rows {
            let bg_color = rgba(255, 255, 255, 255);
            // TODO: proper background fill
            grid.colors[grid.stride * y + x] = Colors {
                fg: 0,
                bg: bg_color,
            };
            grid.chars[grid.stride * y + x] = ' ';
        }
    }

    y + line_height
}

pub fn draw_editor_into_console(
    editor: &LineEditor,
    grid: &mut EmulatorState,
    color: u32,
    blink: bool,
) {
    let selection = editor.selection_range();
    let primary_pos = editor.primary.byte;

    let initial_cursor = grid.cursor;
    grid.scrolled_rows = 0;

    for (i, char) in editor.buf.char_indices() {
        match char {
            '\r' => continue,
            '\n' => {
                if i == primary_pos {
                    // TODO: slightly less weird behavior here (full line, cursor at end)
                    // Maybe make it always wrap a blank line if there isn't width for the cursor?
                    grid.check_wrap();
                    if blink {
                        let bg_color = rgba(255, 255, 255, 255);
                        grid.set_char_color(
                            grid.cursor,
                            ' ',
                            Colors {
                                fg: 0,
                                bg: bg_color,
                            },
                        );
                    }
                }
                grid.wrap();
                continue;
            }
            _ => (),
        }

        let mut colors = Colors {
            fg: color,
            bg: rgba(0, 0, 0, 0),
        };
        if selection.contains(&i) || (i == primary_pos && blink) {
            colors.fg = rgba(0, 0, 0, 255);
            colors.bg = rgba(255, 255, 255, 255);
        }

        grid.set_char_color(grid.cursor, char, colors);
        grid.cursor.col += 1;
        grid.check_wrap();
    }

    if editor.buf.len() == primary_pos {
        let bg_color = if blink {
            rgba(255, 255, 255, 255)
        } else {
            rgba(0, 0, 0, 255)
        };
        grid.check_wrap();
        grid.set_char_color(
            grid.cursor,
            ' ',
            Colors {
                fg: 0,
                bg: bg_color,
            },
        );
    }

    let bg_color = rgba(0, 0, 0, 0);
    for c in grid.cursor.col + 1..grid.cols {
        let pos = GridCoords {
            row: grid.cursor.row,
            col: c,
        };
        grid.set_char_color(
            pos,
            ' ',
            Colors {
                fg: 0,
                bg: bg_color,
            },
        );
    }

    grid.cursor = GridCoords {
        row: initial_cursor.row.saturating_sub(grid.scrolled_rows),
        col: initial_cursor.col,
    };
    grid.scrolled_rows = 0;
}
