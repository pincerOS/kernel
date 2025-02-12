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
opt-level = 2
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
    println!("Hello from child!");

    let _status = sys::send_block(
        chan,
        &sys::Message {
            tag: u64::from_be_bytes(*b"CHILD???"),
            objects: [0; 4],
        },
        &[],
    );

    unsafe { sys::exit() };
}
