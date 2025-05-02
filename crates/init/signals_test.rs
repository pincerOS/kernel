#!/usr/bin/env bash
#![doc = r##"<!-- Absolutely cursed hacks:
NAME="$(basename "$0" .rs)"
DIR=$(cd "$(dirname "$0")" && pwd -P)
ULIB_DIR="$(git rev-parse --show-toplevel)/crates/ulib"
exec "$ULIB_DIR/compile.sh" "$0" <<END_MANIFEST
[package]
name = "$NAME"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "$NAME"
path = "$DIR/$NAME.rs"

[dependencies]
ulib = { path = "$ULIB_DIR" }

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

use ulib::sys::{exit, register_signal_handler, mmap, sys_sigreturn, sys_kill, sys_kill_unblockable, SignalCode};

fn page_fault_handler(_signal_number: u32, fault_addr: *mut ()) {

    println!("Inside of the user page fault handler, page fault at address {:x}", fault_addr as usize);
    let _mmap_addr: *mut u8 =
        unsafe { mmap(0, 4096, 0, 1 << 1, 0, 0).unwrap() } as *mut u8;
    println!("Memory range is mmaped!");
    
    unsafe { sys_sigreturn(); }
    unreachable!();
}

#[no_mangle]
fn main() {

    let _root_fd = 3;
    let page_fault_handler_ptr: fn() = unsafe { core::mem::transmute<fn(u32, *mut ()), fn()>(page_fault_handler) };
    register_signal_handler(SignalCode::PageFault, page_fault_handler_ptr);

    static HELLO_CHARS: [u8; 5] = *b"hello";
    const VIRTUAL_ADDR: usize = 0x1E00000;
    let hello_addr = VIRTUAL_ADDR as *mut u8;
    
    //Writing to unmappred region, user page fault handler should trigger
    unsafe {
        core::ptr::copy_nonoverlapping(
            &raw const HELLO_CHARS[0],
            hello_addr,
            HELLO_CHARS.len(),
        );
    }
    
    for i in 0..5 {
        let curr_char: u8 = unsafe { *(hello_addr.wrapping_add(i)) };
        if curr_char != HELLO_CHARS[i] {
            panic!(
                "incorrect character at index {}! Expected {} got {}",
                i, HELLO_CHARS[i] as char, curr_char as char
            );
        }
    }

    println!("signal handler test succeeded!");
    
    //TODO: add kill and kill unblockable tests

    exit(15);
}
