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
const CMD_CAT:  u64 = 0x0000000000000002;
// later, have CMD_WRITE or CMD_SAVE
#[no_mangle]
fn main(chan: ChannelDesc) {
    let mut buf = [0; 1024];
    let mut line = [0; 1024];
    let mut line_idx = 0;
    println!("Starting editor");
    // my last attempt sucked, I'm gonna rewrite
}
