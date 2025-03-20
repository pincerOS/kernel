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

    println!("Starting mmap test!");
    
    unsafe { sys::mmap(VIRTUAL_ADDR, 4096, true); }
    println!("Done with mmap, writing bytes");    
    let mut virt_ptr: *mut u8 = VIRTUAL_ADDR as *mut u8;
    unsafe { write_bytes(virt_ptr, 'a' as u8, 4096); }
    println!("Bytes written!");
    for _i in 0..4096 {
        virt_ptr = virt_ptr.wrapping_add(1);
        if unsafe { *virt_ptr } != ('a' as u8){
            println!("Error: incorrect value found at address {:p}", virt_ptr);
            unsafe { sys::shutdown(); }
        }
    }
    println!("Bytes verified, unmapping range");
    unsafe { sys::munmap(VIRTUAL_ADDR); }

    let mut a_counter: u32 = 0;
    virt_ptr = VIRTUAL_ADDR as *mut u8;

    unsafe { sys::mmap(VIRTUAL_ADDR, 4096, true); }
    println!("Range mapped again");
    for _i in 0..4096 {
        virt_ptr = virt_ptr.wrapping_add(1);
        if unsafe { *virt_ptr } == ('a' as u8) {
            a_counter += 1;
        }
    }
    println!("Done counting, unmapping again");
    unsafe { sys::munmap(VIRTUAL_ADDR); }

    println!("Found {} 'a' characters in the mmaped range", a_counter);

    println!("Done with mmap test!");
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
