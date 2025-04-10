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

#[no_mangle]
fn main(_chan: ulib::sys::ChannelDesc) {
    let root = 3;
    println!("Listing dir: {}", root);

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
        match ulib::sys::pread(root, data, cookie) {
            Err(e) => {
                println!("Error reading dir: {e}");
                ulib::sys::exit(1);
            },
            Ok(0) => break,
            Ok(len) => {
                let mut i = 0;
                while i < len as usize {
                    let slice = &data[i..];
                    assert!(slice.len() >= size_of::<DirEntry>());
                    let entry = unsafe { *slice.as_ptr().cast::<DirEntry>() };

                    let name_off = core::mem::offset_of!(DirEntry, name);
                    let name = &slice[name_off..][..entry.name_len as usize];
                    let name = core::str::from_utf8(name).unwrap();
                    println!("{}", name);
                    i += entry.rec_len as usize;
                    cookie = entry.next_entry_cookie;
                }
                if cookie == 0 {
                    break;
                }
            }
        }
    }

    ulib::sys::exit(0);
}
