#!/usr/bin/env bash
#![doc = r##"<!-- Absolutely cursed hacks:
SOURCE="$0"
NAME=$(basename "$0" .rs)
DIR="$(cd "$(dirname "$0")" && pwd)"
ULIB_PATH="$(cd "$(dirname "$0")/../ulib" && pwd)"
exec "$(dirname "$0")/../ulib/compile.sh" "$0" <<END_MANIFEST
[package]
name = "$NAME"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "$NAME"
path = "$DIR/$NAME.rs"

[dependencies]
ulib = { path = "$ULIB_PATH" }

[profile.standalone]
inherits = "release"
opt-level = 0
panic = "abort"
strip = "debuginfo"

END_MANIFEST
exit # -->"##]
#![no_std]
#![no_main]

#[macro_use]
extern crate ulib;

use ulib::sys::{recv_block, send_block, ChannelDesc, Message};

const CMD_EXIT: u64 = 0x0000000000000001;
const CMD_CAT: u64 = 0x0000000000000002;
const CMD_LS: u64 = 0x0000000000000003;
const CMD_CD: u64 = 0x0000000000000004;
const CMD_EXEC: u64 = 0x0000000000000005;
const CMD_PWD: u64 = 0x0000000000000006;

fn try_read_stdin(buf: &mut [u8]) -> isize {
    let chan = ChannelDesc(1);
    match recv_block(chan, buf) {
        Ok((i, _)) => i as isize,
        Err(i) => i,
    }
}

struct LineReader {
    buf: [u8; 4096],
    cursor: usize,
    processed: usize,
    cur_base: usize,
}

impl LineReader {
    fn shift(&mut self) {
        if self.cur_base != 0 {
            self.buf[..self.cursor].copy_within(self.cur_base.., 0);
            self.cursor -= self.cur_base;
            self.processed -= self.cur_base;
            self.cur_base = 0;
        }
    }
    
    fn handle_backspace(&mut self, effective_length: &mut usize) {
        if *effective_length > 0 {
            *effective_length -= 1;
            print!("\x08 \x08");
        }
    }
}

fn readline(reader: &mut LineReader) -> Result<&[u8], isize> {
    reader.shift();
    let mut effective_length = 0;
    
    loop {
        while reader.processed < reader.cursor {
            let i = reader.processed;
            reader.processed += 1;
            
            match reader.buf[i] {
                b'\r' => {
                    let base = reader.cur_base;
                    reader.cur_base = i + 1;
                    return Ok(&reader.buf[base..base + effective_length]);
                }
                // Handle backspace (ASCII DEL character)
                b'\x7f' => reader.handle_backspace(&mut effective_length),
                // Also handle Ctrl+H as backspace
                b'\x08' => reader.handle_backspace(&mut effective_length),
                c => {
                    if c.is_ascii_control() && c != b'\t' {
                        print!("^{}", (c + 64) as char);
                    } else {
                        reader.buf[reader.cur_base + effective_length] = c;
                        effective_length += 1;
                        print!("{}", c as char);
                    }
                }
            }
        }

        let read = try_read_stdin(&mut reader.buf[reader.cursor..]);
        match read {
            -2 => continue,
            err @ (..=-1) => return Err(err),
            read @ 0.. => reader.cursor += read as usize,
        }
    }
}

#[no_mangle]
fn main(chan: ChannelDesc) {
    println!("Starting 🐚");

    let mut buf = [0; 1024];
    let (len, msg) = recv_block(chan, &mut buf).unwrap();
    let data = &buf[..len];

    println!(
        "Received message from parent; tag {:#x}, data {:?}",
        msg.tag,
        core::str::from_utf8(data).unwrap()
    );

    let mut reader = LineReader {
        buf: [0; 4096],
        cursor: 0,
        processed: 0,
        cur_base: 0,
    };
    
    let mut response_buf = [0; 1024 * 1024]; // 1MB for huge files
    let mut cwd_buf = [0; 1024];
    
    loop {
        // Get cwd before displaying prompt
        let msg = Message {
            tag: CMD_PWD,
            objects: [0; 4],
        };
        send_block(chan, &msg, b"");
        
        let (len, _) = match recv_block(chan, &mut cwd_buf) {
            Ok(res) => res,
            Err(_) => (0, Message { tag: 0, objects: [0; 4] }),
        };
        
        if len > 0 {
            if let Ok(cwd) = core::str::from_utf8(&cwd_buf[..len]) {
                print!("{} $ ", cwd);
            } else {
                print!("$ ");
            }
        } else {
            print!("$ ");
        }
        
        let line = match readline(&mut reader) {
            Ok(line) => line,
            Err(err) => {
                println!("Error: {err}");
                break;
            }
        };
        println!();

        let line = unsafe { core::str::from_utf8_unchecked(&line) };
        if line.trim().is_empty() {
            continue;
        }

        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");
        
        match cmd {
            "exit" | "quit" => {
                let msg = Message {
                    tag: CMD_EXIT,
                    objects: [0; 4],
                };
                send_block(chan, &msg, b"");
                break;
            },
            "echo" => {
                let rest = line.trim_start_matches("echo").trim();
                println!("{}", rest);
            },
            "pwd" => {
                // Get cwd 
                let msg = Message {
                    tag: CMD_PWD,
                    objects: [0; 4],
                };
                send_block(chan, &msg, b"");
                
                let (len, _) = match recv_block(chan, &mut response_buf) {
                    Ok(res) => res,
                    Err(err) => {
                        println!("Error receiving response: {}", err);
                        continue;
                    }
                };
                
                if len > 0 {
                    if let Ok(content) = core::str::from_utf8(&response_buf[..len]) {
                        println!("{}", content);
                    }
                }
            },
            "cat" => {
                let file = parts.next().unwrap_or("");
                if file.is_empty() {
                    println!("Usage: cat <filename>");
                    continue;
                }
                
                let msg = Message {
                    tag: CMD_CAT,
                    objects: [0; 4],
                };
                send_block(chan, &msg, file.as_bytes());
                
                let (len, _) = match recv_block(chan, &mut response_buf) {
                    Ok(res) => res,
                    Err(err) => {
                        println!("Error receiving response: {}", err);
                        continue;
                    }
                };
                
                if len == 0 {
                    println!("File not found or empty");
                } else {
                    match core::str::from_utf8(&response_buf[..len]) {
                        Ok(content) => println!("{}", content),
                        Err(_) => println!("[Unprintable file content]"),
                    }
                }
            },
            "ls" => {
                let msg = Message {
                    tag: CMD_LS,
                    objects: [0; 4],
                };
                send_block(chan, &msg, b"");
                
                let (len, _) = match recv_block(chan, &mut response_buf) {
                    Ok(res) => res,
                    Err(err) => {
                        println!("Error receiving response: {}", err);
                        continue;
                    }
                };
                
                if len > 0 {
                    if let Ok(content) = core::str::from_utf8(&response_buf[..len]) {
                        println!("{}", content);
                    }
                }
            },
            "cd" => {
                let dir = parts.next().unwrap_or("");
                let msg = Message {
                    tag: CMD_CD,
                    objects: [0; 4], 
                };
                send_block(chan, &msg, dir.as_bytes());
                
                let (len, _) = match recv_block(chan, &mut response_buf) {
                    Ok(res) => res,
                    Err(err) => {
                        println!("Error receiving response: {}", err);
                        continue;
                    }
                };
                
                if len > 0 {
                    if let Ok(content) = core::str::from_utf8(&response_buf[..len]) {
                        println!("{}", content);
                    }
                }
            },
            _ => {
                if cmd.contains('/') || cmd.starts_with("./") {
                    // looks like a file path, try to execute it
                    println!("attempting to execute: {}", cmd);
                    
                    let msg = Message {
                        tag: CMD_EXEC, 
                        objects: [0; 4],
                    };
                    send_block(chan, &msg, cmd.as_bytes());
                    
                    let (len, _) = match recv_block(chan, &mut response_buf) {
                        Ok(res) => res,
                        Err(err) => {
                            println!("Error receiving response: {}", err);
                            continue;
                        }
                    };
                    
                    if len > 0 {
                        if let Ok(content) = core::str::from_utf8(&response_buf[..len]) {
                            println!("{}", content);
                        }
                    }
                } else {
                    println!("Unknown command: {}", cmd);
                }
            }
        }
    }

    unsafe { ulib::sys::exit() };
}
