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

fn try_read_stdin(buf: &mut [u8]) -> isize {
    let stdin_fd = 0;
    unsafe { ulib::sys::pread(stdin_fd, buf.as_mut_ptr(), buf.len(), 0) }
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
fn main(_chan: ulib::sys::ChannelDesc) {
    println!("Starting 🐚");

    let root = 3;
    let path = "test.txt";
    let fd = unsafe { ulib::sys::openat(root, path.len(), path.as_bytes().as_ptr(), 0, 0) };

    println!("File: {}", fd);

    let mut data = [0; 4096];

    let mut read = 0;
    while read < data.len() {
        match unsafe {
            ulib::sys::pread(
                fd as usize,
                data[read..].as_mut_ptr(),
                data[read..].len(),
                read as u64,
            )
        } {
            (..=-1) => break,
            i @ (1..) => read += i as usize,
            0 => break,
        }
    }

    println!(
        "File content: ======\n{}\n====================",
        core::str::from_utf8(&data[..read]).unwrap()
    );

    let stdout = 1;
    let buf = b"Stdout write test\n";
    unsafe { ulib::sys::pwrite_all(stdout, buf, 0) };

    println!("Dir: {}", root);

    let mut cookie = 0;
    let mut data_backing = [0u64; 8192 / 8];
    let data = cast_slice(&mut data_backing);

    fn cast_slice<'a>(s: &'a mut [u64]) -> &'a mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(s.as_mut_ptr().cast::<u8>(), s.len() * size_of::<u64>())
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub struct DirEntry {
        pub inode: u64,
        pub next_entry_cookie: u64,
        pub rec_len: u16,
        pub name_len: u16,
        pub file_type: u8,
        pub name: [u8; 3],
        // Name is an arbitrary size array; the record is always padded with
        // 0 bytes such that rec_len is a multiple of 8 bytes.
    }

    loop {
        println!("reading dir with cookie {}", cookie);
        match unsafe { ulib::sys::pread(root, data.as_mut_ptr(), data.len(), cookie) } {
            (..=-1) => break,
            len @ (1..) => {
                println!("read {} bytes", len);
                let mut i = 0;
                while i < len as usize {
                    let slice = &data[i..];
                    assert!(slice.len() >= size_of::<DirEntry>());
                    let entry = unsafe { *slice.as_ptr().cast::<DirEntry>() };
                    println!("Entry: {:#?}", entry);
                    let name =
                        &slice[core::mem::offset_of!(DirEntry, name)..][..entry.name_len as usize];
                    println!("Name: {}", core::str::from_utf8(name).unwrap());
                    i += entry.rec_len as usize;
                    cookie = entry.next_entry_cookie;
                }
                if cookie == 0 {
                    break;
                }
            }
            0 => {
                println!("read 0 bytes, exiting");
                break;
            }
        }
    }

    // let mut buf = [0; 1024];
    // let (len, msg) = recv_block(chan, &mut buf).unwrap();
    // let data = &buf[..len];

    // println!(
    //     "Received message from parent; tag {:#x}, data {:?}",
    //     msg.tag,
    //     core::str::from_utf8(data).unwrap()
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

        println!("Got line: {}", line);

        // let msg = Message {
        //     tag: 0xAAAAAAAA,
        //     objects: [0; 4],
        // };
        // send_block(chan, &msg, line.as_bytes());

        for _ in 0..100 {
            // TODO: this is a hack to prevent concurrent access to stdout...
            unsafe { ulib::sys::yield_() }
        }
    }

    unsafe { ulib::sys::exit() };
}
