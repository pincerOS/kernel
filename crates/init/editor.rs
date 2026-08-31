#![no_std]
#![no_main]

extern crate ulib;
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::borrow::ToOwned;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    Insert,
}

struct Editor {
    buffer: Vec<String>,
    row: usize,
    col: usize,
    mode: Mode,
}

impl Editor {
    fn new() -> Self {
        let mut buffer = Vec::new();
        buffer.push(String::new());
        Editor { buffer, row: 0, col: 0, mode: Mode::Normal }
    }

    fn insert_char(&mut self, ch: char) {
        if let Some(line) = self.buffer.get_mut(self.row) {
            line.insert(self.col, ch);
            self.col += 1;
            print!("{}", ch);
        }
    }

    fn insert_newline(&mut self) {
        if let Some(current_line) = self.buffer.get_mut(self.row) {
            let tail = if self.col < current_line.len() {
                let tail_part = current_line[self.col..].to_owned();
                current_line.truncate(self.col);
                tail_part
            } else {
                String::new()
            };
            self.row += 1;
            self.col = 0;
            self.buffer.insert(self.row, tail);
            print!("\n");
        }
    }

    fn backspace(&mut self) {
        if self.col > 0 {
            if let Some(line) = self.buffer.get_mut(self.row) {
                line.remove(self.col - 1);
            }
            self.col -= 1;
            print!("\x08 \x08");
        } else if self.row > 0 {
            let current_line = self.buffer.remove(self.row);
            self.row -= 1;
            if let Some(prev_line) = self.buffer.get_mut(self.row) {
                let prev_len = prev_line.len();
                prev_line.push_str(&current_line);
                self.col = prev_len;
                print!("\r");
                print!("{}", prev_line);
            } else {
                self.col = 0;
            }
        }
    }
}

fn save(buffer: &Vec<String>) {
    let _ = buffer;
}

fn readline() -> String {
    let mut line = String::new();
    loop {
        let mut buf = [0u8; 1];
        let n = unsafe { recv_block(1, buf.as_mut_ptr(), 1) };
        if n <= 0 {
            continue;
        }
        let byte = buf[0];
        if byte == b'\n' || byte == b'\r' {
            break;
        }
        let ch = byte as char;
        line.push(ch);
        print!("{}", ch);
    }
    line
}

extern "C" {
    fn recv_block(desc: u32, buf: *mut u8, len: usize) -> isize;
}

#[no_mangle]
pub extern "C" fn main() -> i32 {
    let mut editor = Editor::new();
    loop {
        let mut byte_buf = [0u8; 1];
        let res = unsafe { recv_block(1, byte_buf.as_mut_ptr(), 1) };
        if res <= 0 {
            continue;
        }
        let ch = byte_buf[0] as char;
        match editor.mode {
            Mode::Normal => {
                match ch {
                    'h' => {
                        if editor.col > 0 {
                            editor.col -= 1;
                            print!("\x08");
                        }
                    }
                    'l' => {
                        if let Some(line) = editor.buffer.get(editor.row) {
                            if line.len() > 0 && editor.col < line.len() - 1 {
                                editor.col += 1;
                                let c = line.chars().nth(editor.col).unwrap();
                                print!("{}", c);
                            }
                        }
                    }
                    'k' => {
                        if editor.row > 0 {
                            editor.row -= 1;
                            let line = &editor.buffer[editor.row];
                            if editor.col > line.len() {
                                editor.col = if line.len() > 0 { line.len() - 1 } else { 0 };
                            }
                            print!("\r");
                            print!("{}", line);
                        }
                    }
                    'j' => {
                        if editor.row + 1 < editor.buffer.len() {
                            editor.row += 1;
                            let line = &editor.buffer[editor.row];
                            if editor.col > line.len() {
                                editor.col = if line.len() > 0 { line.len() - 1 } else { 0 };
                            }
                            print!("\r");
                            print!("{}", line);
                        }
                    }
                    'i' => {
                        editor.mode = Mode::Insert;
                    }
                    ':' => {
                        print!("\n:");
                        let cmd = readline();
                        if cmd == "q" {
                            break;
                        } else if cmd == "w" {
                            save(&editor.buffer);
                        } else if cmd == "wq" || cmd == "qw" {
                            save(&editor.buffer);
                            break;
                        }
                        print!("\n");
                        if let Some(line) = editor.buffer.get(editor.row) {
                            print!("{}", line);
                        }
                    }
                    _ => {}
                }
            }
            Mode::Insert => {
                match ch {
                    '\x1B' => {
                        editor.mode = Mode::Normal;
                        if editor.col > 0 {
                            editor.col -= 1;
                        }
                    }
                    '\n' | '\r' => {
                        editor.insert_newline();
                    }
                    '\x08' | '\x7F' => {
                        editor.backspace();
                    }
                    _ => {
                        if !ch.is_control() && ch.is_ascii() {
                            editor.insert_char(ch);
                        }
                    }
                }
            }
        }
    }

    0
}

use core::panic::PanicInfo;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
