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

use ulib::sys::{openat, close, mmap, munmap, wait, exit, MAP_PRIVATE, MAP_SHARED, MAP_FILE};

#[no_mangle]
fn main() {

    let root_fd = 3;

    let test_file_path = b"test.txt";
    let mut test_text_file = openat(root_fd, test_file_path, 0, 0).unwrap();

    static HELLO_CHARS: [u8; 5] = *b"hello";
    static WORLD_CHARS: [u8; 5] = *b"world";
    
    let mut mmap_addr: *mut u8 =
        unsafe { mmap(0, 4096, 0, MAP_PRIVATE | MAP_FILE, test_text_file, 0).unwrap() } as *mut u8;
    println!("Memory range is mmaped!");

    for i in 0..5 {
        let curr_char: u8 = unsafe { *(mmap_addr.wrapping_add(i)) };
        if curr_char != HELLO_CHARS[i] {
            panic!(
                "mmap filed file has an error at index {}! Expected {} got {}",
                i, HELLO_CHARS[i] as char, curr_char as char
            );
        }
    }

    println!("mmap of test file succeeded!");
    unsafe { munmap(mmap_addr as *mut ()).unwrap() };
    _ = close(test_text_file);

    println!("starting mmap with offset test 1");
    let test_file_path2 = b"test2.txt";
    test_text_file = openat(root_fd, test_file_path2, 0, 0).unwrap();
    mmap_addr =
        unsafe { mmap(0, 4096 * 2, 0, MAP_PRIVATE | MAP_FILE, test_text_file, 6).unwrap() } as *mut u8;
    println!("mmap addr: {:x}", mmap_addr as usize);

    for i in 0..5 {
        let curr_char: u8 = unsafe { *(mmap_addr.wrapping_add(i)) };
        if curr_char != WORLD_CHARS[i] {
            panic!(
                "mmap filed file has an error at index {}! Expected {} got {}",
                i, WORLD_CHARS[i] as char, curr_char as char
            );
        }
    }

    println!("done with mmap with offset test 1");

    println!("starting mmap with offset test 2");
    for i in 0..5 {
        let curr_char: u8 = unsafe { *(mmap_addr.wrapping_add(i + 4096)) };
        if curr_char != WORLD_CHARS[i] {
            panic!(
                "mmap filed file has an error at index {}! Expected {} got {}",
                i, WORLD_CHARS[i] as char, curr_char as char
            );
        }
    }

    println!("done with mmap with offset test 2");
    unsafe { munmap(mmap_addr as *mut ()).unwrap() };
    _ = close(test_text_file);

    println!("Starting shared memory test");

    let shared_mem_fd = unsafe { ulib::sys::sys_memfd_create() as u32 };
    let shared_frame =
        unsafe { ulib::sys::mmap(0, 4096, 0, MAP_SHARED, shared_mem_fd, 0) }.unwrap() as *mut u8;
    let end_ptr: *mut u8 = shared_frame.wrapping_add(6);
    unsafe { end_ptr.write_volatile('.' as u8) };
    assert_eq!(unsafe { end_ptr.read_volatile() }, '.' as u8);

    println!("Calling fork");
    let wait_fd = unsafe { ulib::sys::sys_fork() } as u32;

    if wait_fd == 0 {
        for _i in 0..1000000 {
            if unsafe { end_ptr.read_volatile() } == ('!' as u8) {
                break;
            }
        }

        let end_val: u8 = unsafe { end_ptr.read_volatile() };
        if end_val != ('!' as u8) {
            println!(
                "Error: child expected to see '!' with val {} and instead found {}",
                ('!' as u8),
                end_val
            );
            ulib::sys::exit(5);
        }

        for i in 0..5 {
            let curr_char = unsafe { *shared_frame.wrapping_add(i) } as char;
            if curr_char != (HELLO_CHARS[i] as char) {
                println!(
                    "Child found wrong char at index {}. Expected {} found {}",
                    i,
                    (HELLO_CHARS[i] as char),
                    curr_char
                );
                ulib::sys::exit(5);
            }
        }

        println!("Child is done");
        ulib::sys::exit(0);
    } else {
        unsafe {
            core::ptr::copy_nonoverlapping(
                &raw const HELLO_CHARS[0],
                shared_frame,
                HELLO_CHARS.len(),
            );
            shared_frame.wrapping_add(6).write('!' as u8);
        }

        let child_exit_val = wait(wait_fd).unwrap();
        assert_eq!(child_exit_val, 0);
        println!("done with shared memory test, parent received exit code 0 from child");
    }

    _ = unsafe { munmap(shared_frame as *mut ()) };

    exit(15);
}
