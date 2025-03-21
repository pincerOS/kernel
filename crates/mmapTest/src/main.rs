#![no_std]
#![no_main]

#[macro_use]
extern crate ulib;

mod runtime;

use ulib::sys;
use core::ptr::write_bytes;

const VIRTUAL_ADDR: usize = 0x1E00000;

#[no_mangle]
pub extern "C" fn main() {

    println!("Starting specified addr mmap test!");
    
    let mut mmap_addr: usize = unsafe { sys::mmap(VIRTUAL_ADDR, 4096, true) };
    if mmap_addr == usize::MAX {
        println!("Error: mmap failed!");
        unsafe { sys::shutdown(); }
        unreachable!();
    }

    println!("Done with mmap, writing bytes");    
    let mut virt_ptr: *mut u8 = VIRTUAL_ADDR as *mut u8;
    unsafe { write_bytes(virt_ptr, 'a' as u8, 4096); }
    println!("Bytes written!");
    //println!("Value of virt_ptr: {}", virt_ptr as usize);
    for _i in 0..4096 {
        if unsafe { *virt_ptr } != ('a' as u8){
            println!("Error: incorrect value found at address {:p}", virt_ptr);
            unsafe { sys::shutdown(); }
            unreachable!();
        }
        virt_ptr = virt_ptr.wrapping_add(1);
    }
    println!("Bytes verified, unmapping range");
    let mut munmap_ret_val = unsafe { sys::munmap(VIRTUAL_ADDR) };
    if munmap_ret_val != 0 {
        println!("Error: munmap return value is {}", munmap_ret_val);
        unsafe { sys::shutdown(); }
        unreachable!();
    }

    let mut a_counter: u32 = 0;
    virt_ptr = VIRTUAL_ADDR as *mut u8;

    mmap_addr = unsafe { sys::mmap(VIRTUAL_ADDR, 4096, true) };
    if mmap_addr == usize::MAX {
        println!("Error: mmap failed!");
        unsafe { sys::shutdown(); }
        unreachable!();
    }
    
    println!("Range mapped again");
    for _i in 0..4096 {
        if unsafe { *virt_ptr } == ('a' as u8) {
            a_counter += 1;
        }
        virt_ptr = virt_ptr.wrapping_add(1);
    }
    println!("Done counting, unmapping again");
    munmap_ret_val = unsafe { sys::munmap(VIRTUAL_ADDR) };
    if munmap_ret_val != 0 {
        println!("Error: munmap return value is {}", munmap_ret_val);
        unsafe { sys::shutdown(); }
        unreachable!();
    }

    println!("Found {} 'a' characters in the mmaped range", a_counter);

    println!("Done with specified addrm map test!");
    
    println!("Testing unspecified mmap");
    let first_unspec_addr: usize = unsafe { sys::mmap(0, 4096, true)  };
    if first_unspec_addr == usize::MAX {
        println!("Error: mmap failed!");
        unsafe { sys::shutdown(); }
        unreachable!();
    }

    munmap_ret_val = unsafe { sys::munmap(first_unspec_addr) };
    if munmap_ret_val != 0 {
        println!("Error: munmap return value is {}", munmap_ret_val);
        unsafe { sys::shutdown(); }
        unreachable!();
    }

    let second_unspec_addr: usize = unsafe { sys::mmap(0, 4096, true)  };
    if second_unspec_addr == usize::MAX {
        println!("Error: mmap failed!");
        unsafe { sys::shutdown(); }
        unreachable!();
    }
    
    munmap_ret_val = unsafe { sys::munmap(second_unspec_addr) };
    if munmap_ret_val != 0 {
        println!("Error: munmap return value is {}", munmap_ret_val);
        unsafe { sys::shutdown(); }
        unreachable!();
    }

    if first_unspec_addr != second_unspec_addr {
        println!("Unspecifed mmap addrs do not match! First addr: {:x} Second addr: {:x}", first_unspec_addr, second_unspec_addr);
        unsafe { sys::shutdown(); }
        unreachable!();
    }

    println!("Unspecified mmap works as intended!");

    unsafe { sys::shutdown() };
    unreachable!();
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
