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

fn iter_args(mut data: &[u8]) -> impl Iterator<Item = &[u8]> + '_ {
    core::iter::from_fn(move || {
        if data.len() == 0 {
            return None;
        }
        let idx = data
            .iter()
            .position(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
            .unwrap_or(data.len());
        let (arg, rest) = data.split_at(idx);

        let idx = rest
            .iter()
            .position(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
            .unwrap_or(rest.len());
        data = &rest[idx..];
        Some(arg)
    })
}

#[no_mangle]
fn main(chan: sys::ChannelDesc) {
    let mut args = [0; 4096];
    let (len, msg) = sys::recv_block(chan, &mut args).unwrap();
    assert_eq!(msg.tag, u64::from_be_bytes(*b"ARGS----"));
    let data = &args[..len];

    for arg in iter_args(data) {
        println!("Arg {}", ulib::format_ascii(arg));
    }

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
