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

#[repr(C)]
#[derive(Copy, Clone)]
struct ReadAt {
    file_id: u32,
    amount: u32,
    offset: u64,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Open {
    path_len: u32,
}

fn spawn_child(files: ChannelDesc, procs: ChannelDesc, file_path: &str) -> ChannelDesc {
    let mut buf = [0; 512];
    buf[..4].copy_from_slice(&u32::to_le_bytes(file_path.len() as u32));
    buf[4..][..file_path.len()].copy_from_slice(file_path.as_bytes());

    let _status = send_block(
        files,
        &Message {
            tag: u64::from_be_bytes(*b"OPEN----"),
            objects: [0; 4],
        },
        &buf[..4 + file_path.len()],
    );

    let mut file_id = [0u8; 4];
    let (_, msg) = recv_block(files, &mut file_id).unwrap();
    assert_eq!(msg.tag, u64::from_be_bytes(*b"OPENSUCC"));
    let file_id = u32::from_le_bytes(file_id);

    let _status = send_block(
        procs,
        &Message {
            tag: u64::from_be_bytes(*b"SPAWN---"),
            objects: [0; 4],
        },
        &u32::to_le_bytes(file_id),
    );

    let (_, msg) = recv_block(procs, &mut []).unwrap();
    assert_eq!(msg.tag, u64::from_be_bytes(*b"SUCCESS-"));
    let child_handle = ChannelDesc(msg.objects[0]);
    child_handle
}

#[no_mangle]
fn main(chan: ChannelDesc) {
    println!("Starting 🐚");

    let _status = send_block(
        chan,
        &Message {
            tag: u64::from_be_bytes(*b"CONNREQ-"),
            objects: [0; 4],
        },
        b"FILES---",
    );

    let (_, msg) = recv_block(chan, &mut []).unwrap();
    let filesystem = ChannelDesc(msg.objects[0]);

    let _status = send_block(
        filesystem,
        &Message {
            tag: u64::from_be_bytes(*b"OPEN----"),
            objects: [0; 4],
        },
        "\x08\x00\x00\x00test.txt".as_bytes(),
    );

    let mut file_id = [0u8; 4];
    let (_, msg) = recv_block(filesystem, &mut file_id).unwrap();
    assert_eq!(msg.tag, u64::from_be_bytes(*b"OPENSUCC"));
    let file_id = u32::from_le_bytes(file_id);

    let read = ReadAt {
        file_id,
        amount: 4096,
        offset: 0,
    };
    let _status = send_block(
        filesystem,
        &Message {
            tag: u64::from_be_bytes(*b"READAT--"),
            objects: [0; 4],
        },
        unsafe { core::slice::from_raw_parts(&read as *const _ as *const u8, size_of::<ReadAt>()) },
    );

    let mut data = [0u8; 4096];
    let (len, msg) = recv_block(filesystem, &mut data).unwrap();
    assert_eq!(msg.tag, u64::from_be_bytes(*b"DATA----"));
    let file_content = &data[..len as usize];

    println!(
        "File content:\n{}",
        core::str::from_utf8(file_content).unwrap()
    );

    println!("Attempting to spawn child");

    let _status = send_block(
        chan,
        &Message {
            tag: u64::from_be_bytes(*b"CONNREQ-"),
            objects: [0; 4],
        },
        b"PROCS---",
    );

    let (_, msg) = recv_block(chan, &mut []).unwrap();
    let procs = ChannelDesc(msg.objects[0]);

    let child_handle = spawn_child(filesystem, procs, "hello.elf");

    let (_, msg) = recv_block(child_handle, &mut []).unwrap();
    println!(
        "from child: {}",
        core::str::from_utf8(&msg.tag.to_be_bytes()).unwrap()
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

        println!("Line: {:?}", line);
    }

    unsafe { ulib::sys::exit() };
}
