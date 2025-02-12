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

use ulib::sys;

#[no_mangle]
fn main(chan: sys::ChannelDesc) {
    let mut buf = [0; 1024];
    let (len, msg) = sys::recv_block(chan, &mut buf).unwrap();
    let data = &buf[..len];

    println!(
        "Received message from parent; tag {:#x}, data {:?}",
        msg.tag,
        core::str::from_utf8(data).unwrap()
    );

    for i in 0..10 {
        println!("Running in usermode! {}", i);
    }

    let msg = sys::Message {
        tag: 0xFF00FF00,
        objects: [0; 4],
    };
    sys::send_block(chan, &msg, b"Hello parent!");

    unsafe { ulib::sys::exit() };
}
