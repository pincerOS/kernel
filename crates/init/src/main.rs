#![no_std]
#![no_main]

#[macro_use]
extern crate ulib;

mod runtime;

use ulib::sys::{self, spawn_elf, sys_memfd_create, SpawnArgs};

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    println!("Running in usermode! (parent)");

    let root_fd = 3;
    let path = b"shell.elf";
    let file = ulib::sys::openat(root_fd, path, 0, 0).unwrap();
    
    let test_file_path = b"test.txt";
    let test_text_file = ulib::sys::openat(root_fd, test_file_path, 0, 0).unwrap();

    static HELLO_CHARS: [u8; 5] = *b"hello";
    //TODO: add mmap options flags to ulib 
    let mmap_addr: *mut u8 = unsafe { ulib::sys::mmap(0, 4096, 0, 0, test_text_file, 0).unwrap() } as *mut u8;
    println!("Memory range is mmaped!");    
    
    for i in 0..5 {
        let curr_char: u8 = unsafe { *(mmap_addr.wrapping_add(i)) };
        if curr_char != HELLO_CHARS[i] {
            panic!("mmap filed file has an error at index {}! Expected {} got {}", i, HELLO_CHARS[i], curr_char);
        }
    }

    println!("mmap of test file succeeded!");

    unsafe { sys::munmap(mmap_addr as *mut ()).unwrap() };
    
    ///*
    println!("Starting shared memory test");

    let shared_mem_fd = unsafe { sys_memfd_create() as u32 };
    let shared_frame = unsafe { ulib::sys::mmap(0, 4096, 0, 1 << 2, shared_mem_fd, 0) }.unwrap() as *mut u8;
    let excl_ptr: *mut u8 = shared_frame.wrapping_add(6);
    unsafe { excl_ptr.write('.' as u8) };

    println!("Calling fork");
    let wait_fd = unsafe { ulib::sys::sys_fork() } as u32;
    
    ///*

    if wait_fd == 0 {
        println!("In child");
        for _i in 0..100000 {
            
            if unsafe { excl_ptr.read_volatile() } != ('.' as u8) {
                break;
            }
        }

        if unsafe { excl_ptr.read_volatile() } != ('!' as u8) {
            ulib::sys::exit(5);
        }
        
        for i in 0..5 {
            let curr_char = unsafe { *shared_frame.wrapping_add(i) } as char;
            if curr_char != (HELLO_CHARS[i] as char) {
                println!("Child found wrong char at index {}. Expected {} found {}", i, (HELLO_CHARS[i] as char), curr_char);
                ulib::sys::exit(5);
            }
        }

        println!("Child is done");
        ulib::sys::exit(0);

    } else {
        println!("In parent");
        unsafe {
            core::ptr::copy_nonoverlapping(&raw const HELLO_CHARS[0], shared_frame, HELLO_CHARS.len());
            shared_frame.wrapping_add(6).write('!' as u8);
        }

        unsafe { excl_ptr.write('!' as u8) };
        let child_exit_val = ulib::sys::wait(wait_fd).unwrap();
        assert!(child_exit_val == 0);

    }
    
    //TODO: unmap shared frame
    //*/

    let child = spawn_elf(&ulib::sys::SpawnArgs {
        fd: file,
        stdin: None,
        stdout: None,
    })
    .unwrap();

    let status = ulib::sys::wait(child).unwrap();

    println!("Child exited with status {}", status);

    ulib::sys::shutdown();
}

#[macro_use]
#[doc(hidden)]
pub mod macros {
    #[repr(C)]
    pub struct AlignedAs<Align, Bytes: ?Sized> {
        #[allow(clippy::pub_underscore_fields)]
        pub _align: [Align; 0],
        pub bytes: Bytes,
    }
    #[doc(hidden)]
    #[macro_export]
    macro_rules! __include_bytes_align {
        ($align_ty:ty, $path:literal) => {{
            use $crate::macros::AlignedAs;
            static ALIGNED: &AlignedAs<$align_ty, [u8]> = &AlignedAs {
                _align: [],
                bytes: *include_bytes!($path),
            };
            &ALIGNED.bytes
        }};
    }
}

#[doc(inline)]
pub use crate::__include_bytes_align as include_bytes_align;
