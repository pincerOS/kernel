#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;

#[macro_use]
extern crate ulib;

use alloc::vec::Vec;
use ulib::sys::{dup3, mmap, recv_nonblock, send, FileDesc};

#[no_mangle]
fn main() {
    
}
