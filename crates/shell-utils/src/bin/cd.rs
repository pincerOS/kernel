#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;

#[macro_use]
extern crate ulib;

use alloc::vec::Vec;
use ulib::sys::{dup3, mmap, recv_nonblock, send, FileDesc, chdir};

#[no_mangle]
fn main() {
    //chnage chdir to ./test_folder
    let path = b"test_folder";
    let res = chdir(path);
    if res.is_err() {
        println!("cd: no such file or directory: {:?}", path);
    }
}
