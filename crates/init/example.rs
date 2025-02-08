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

use ulib::sys::{recv_block, send_block, ChannelDesc, Message};

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
}

fn readline(reader: &mut LineReader) -> Result<&[u8], isize> {
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
                b'\x7f' => print!("^?"),
                c if c.is_ascii_control() => print!("^{}", (c + 64) as char),
                c => print!("{}", c as char),
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

        let msg = Message {
            tag: 0xAAAAAAAA,
            objects: [0; 4],
        };
        send_block(chan, &msg, line.as_bytes());

        for _ in 0..100 {
            // TODO: this is a hack to prevent concurrent access to stdout...
            unsafe { ulib::sys::yield_() }
        }
    }

    unsafe { ulib::sys::exit() };
}
