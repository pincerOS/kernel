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
                // b'\x7f' => print!("^?"),
                b'\x7f' => {
                    if reader.processed >= 2 {
                        reader.processed -= 2;
                        reader.cursor -= 2;
                        print!("\x08 \x08");
                    } else {
                        reader.processed -= 1;
                        reader.cursor -= 1;
                    }
                },

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
// #[repr(C)]
// #[derive(Copy, Clone)]
// struct Open {
//     path_len: u32,
// }

fn open_file(files: ChannelDesc, file_path: &[u8]) -> Option<u32> {
    let mut buf = [0; 512];
    buf[..4].copy_from_slice(&u32::to_le_bytes(file_path.len() as u32));
    buf[4..][..file_path.len()].copy_from_slice(file_path);

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
    if msg.tag != u64::from_be_bytes(*b"OPENSUCC") {
        return None;
    }
    let file_id = u32::from_le_bytes(file_id);
    Some(file_id)
}

fn spawn_child(procs: ChannelDesc, exec_file_id: u32) -> ChannelDesc {
    let _status = send_block(
        procs,
        &Message {
            tag: u64::from_be_bytes(*b"SPAWN---"),
            objects: [0; 4],
        },
        &u32::to_le_bytes(exec_file_id),
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

    let file_id = open_file(filesystem, b"test.txt").unwrap();

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

    print!(
        "File content:\n{}",
        core::str::from_utf8(file_content).unwrap()
    );

    // println!("Attempting to spawn child");

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

    // let child_exec = open_file(filesystem, "hello.elf").unwrap();
    // let child_handle = spawn_child(procs, child_exec);

    // let (_, msg) = recv_block(child_handle, &mut []).unwrap();
    // println!(
    //     "from child: {}",
    //     core::str::from_utf8(&msg.tag.to_be_bytes()).unwrap()
    // );

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

        let (cmd, rest) = line.split_once(|c: char| c.is_ascii_whitespace()).unzip();
        let cmd = cmd.unwrap_or(line);

        if cmd == "exit" {
            break;
        }

        let mut buf = [0u8; 64];
        buf[..cmd.len()].copy_from_slice(cmd.as_bytes());
        buf[cmd.len()..][..4].copy_from_slice(b".elf");
        let cmd_with_elf = &buf[..cmd.len() + 4];

        let file = open_file(filesystem, cmd.as_bytes())
            .or_else(|| open_file(filesystem, cmd_with_elf));

        // println!("Line: {:?}", (cmd, rest));
        if let Some(child_exec) = file {
            let child_handle = spawn_child(procs, child_exec);
            let msg = Message {
                tag: u64::from_be_bytes(*b"ARGS----"),
                objects: [0; 4],
            };
            send_block(child_handle, &msg, rest.unwrap_or("").as_bytes());

            // TODO: proper control channels, waitpid
            let (_, _msg) = recv_block(child_handle, &mut []).unwrap();
        } else {
            println!("{:?}: no such file or directory", cmd);
        }
    }

    unsafe { ulib::sys::exit() };
}

// fn split_args(s: &str) {
//     let mut i = 0;
//     let mut bytes = s.as_bytes();
//     while let Some(c) = bytes.get(i) {
//         match c {
//             b'"' => {
//                 for j in i + 1 .. bytes.len() {
//                     if bytes[j] == b'"' && bytes[] {

//                     }
//                 }
//             },
//             b' ' | b'\t' | b'\n' | b'\r' => (),
//         }
//     }
// }
