#!/usr/bin/env bash
#![doc = r##"<!-- Absolutely cursed hacks:
SOURCE="$0" NAME=$(basename "$0" .rs) DIR=$(realpath $(dirname "$0"))
exec "$(dirname "$0")/../ulib/compile.sh" "$0" <<END_MANIFEST
[package]
name = "$NAME"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "$NAME"
path = "$DIR/$NAME.rs"

[dependencies]
ulib = { path = "$DIR/../ulib" }

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

use ulib::sys::FileDesc;

const STDIN_FD: FileDesc = 0;

fn try_read_stdin(buf: &mut [u8]) -> Result<usize, usize> {
    ulib::sys::pread(STDIN_FD, buf, 0)
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
}

fn readline(reader: &mut LineReader) -> Result<&[u8], usize> {
    reader.shift();
    loop {
        while reader.processed < reader.cursor {
            let i = reader.processed;
            reader.processed += 1;
            match reader.buf[i] {
                b'\r' => {
                    let base = reader.cur_base;
                    reader.cur_base = i + 1;
                    return Ok(&reader.buf[base..i]);
                }
                b'\x7f' => {
                    if reader.processed >= 2 {
                        reader.processed -= 2;
                        reader.cursor -= 2;
                        print!("\x08 \x08");
                    } else {
                        reader.processed -= 1;
                        reader.cursor -= 1;
                    }
                }
                c if c.is_ascii_control() => print!("^{}", (c + 64) as char),
                c => print!("{}", c as char),
            }
        }

        let read = try_read_stdin(&mut reader.buf[reader.cursor..])?;
        reader.cursor += read;
    }
}

#[no_mangle]
fn main() {
    println!("Starting 🐚");

    let root = 3;
    let path = "test.txt";
    let fd = ulib::sys::openat(root, path.as_bytes(), 0, 0).unwrap();

    println!("File: {}", fd);

    let mut data = [0; 4096];

    let mut read = 0;
    while read < data.len() {
        match ulib::sys::pread(fd, &mut data[read..], read as u64) {
            Err(_) => break,
            Ok(0) => break,
            Ok(i) => read += i,
        }
    }

    println!(
        "File content: ======\n{}\n====================",
        core::str::from_utf8(&data[..read]).unwrap()
    );

    let stdout = 1;
    let buf = b"Stdout write test\n";
    ulib::sys::pwrite_all(stdout, buf, 0).unwrap();

    let mut reader = LineReader {
        buf: [0; 4096],
        cursor: 0,
        processed: 0,
        cur_base: 0,
    };

    loop {
        print!("$ ");
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

        if line == "exit" {
            break;
        } else {
            let first = line.split_ascii_whitespace().next().unwrap_or(line);
            let root_fd = 3;
            if let Ok(file) = ulib::sys::openat(root_fd, first.as_bytes(), 0, 0) {
                // TODO: args
                let child = ulib::sys::spawn_elf(&ulib::sys::SpawnArgs {
                    fd: file,
                    stdin: None,
                    stdout: None,
                })
                .unwrap();

                if !line.ends_with("&") {
                    let status = ulib::sys::wait(child).unwrap();
                    println!("child exited with code {}", status);
                }
            } else {
                println!("unknown command: {:?}", first);
            }
        }
    }

    ulib::sys::exit(15);
}
