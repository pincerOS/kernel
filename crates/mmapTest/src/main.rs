#![no_std]
#![no_main]

#[macro_use]
extern crate ulib;

mod runtime;

use core::ptr::write_bytes;
use ulib::sys;

const VIRTUAL_ADDR: usize = 0x1E00000;

#[no_mangle]
pub extern "C" fn main() {
    println!("Starting specified addr mmap test!");

    //prot flags and offset are currently not used
    let mut mmap_addr: usize = sys::mmap(VIRTUAL_ADDR, 4096, 0, 1, u32::MAX, 0).unwrap();

    //This doesn't always need to hold but in this case it does since the virtual addr is page
    //aligned
    if mmap_addr != VIRTUAL_ADDR {
        println!("Error: mmap addr is not the same as the requested addr!");
        sys::shutdown();
    }

    println!("Done with mmap, writing bytes");
    let mut virt_ptr: *mut u8 = VIRTUAL_ADDR as *mut u8;
    unsafe {
        write_bytes(virt_ptr, 'a' as u8, 4096);
    }
    println!("Bytes written!");
    //println!("Value of virt_ptr: {}", virt_ptr as usize);
    for _i in 0..4096 {
        if unsafe { *virt_ptr } != ('a' as u8) {
            println!("Error: incorrect value found at address {:p}", virt_ptr);
            sys::shutdown();
        }
        virt_ptr = virt_ptr.wrapping_add(1);
    }
    println!("Bytes verified, unmapping range");
    //length parameter is not used
    _ = sys::munmap(VIRTUAL_ADDR, 0).unwrap();

    let mut a_counter: u32 = 0;
    virt_ptr = VIRTUAL_ADDR as *mut u8;

    mmap_addr = sys::mmap(VIRTUAL_ADDR, 4096, 0, 1, u32::MAX, 0).unwrap();

    if mmap_addr != VIRTUAL_ADDR {
        println!("Error: mmap addr is not the same as the requested addr!");
        sys::shutdown();
    }

    println!("Range mapped again");
    for _i in 0..4096 {
        if unsafe { *virt_ptr } == ('a' as u8) {
            a_counter += 1;
        }
        virt_ptr = virt_ptr.wrapping_add(1);
    }
    println!("Done counting, unmapping again");
    _ = sys::munmap(VIRTUAL_ADDR, 0).unwrap();

    println!("Found {} 'a' characters in the mmaped range", a_counter);

    println!("Done with specified addrm map test!");

    println!("Testing unspecified mmap");
    let first_unspec_addr: usize = sys::mmap(0, 4096, 0, 1, u32::MAX, 0).unwrap();

    _ = sys::munmap(first_unspec_addr, 0).unwrap();

    let second_unspec_addr: usize = sys::mmap(0, 4096, 0, 1, u32::MAX, 0).unwrap();

    _ = sys::munmap(second_unspec_addr, 0).unwrap();

    if first_unspec_addr != second_unspec_addr {
        println!(
            "Unspecifed mmap addrs do not match! First addr: {:x} Second addr: {:x}",
            first_unspec_addr, second_unspec_addr
        );
        sys::shutdown();
    }

    println!("Unspecified mmap works as intended!");

    println!("Starting physical range mmap test!");
    let phys_addr: usize = 0x5ef000;
    println!("Physical addr used: {:x}. Ensure that the kernel part of this test case is leaking this physical addr", phys_addr);

    //Don't want to fill pages!
    mmap_addr = sys::mmap(0, 4096 * 2, 0, 0, u32::MAX, 0).unwrap();

    _ = sys::map_physical(mmap_addr, phys_addr).unwrap();
    println!("Associated virtual addr: {:x}", mmap_addr);
    virt_ptr = mmap_addr as *mut u8;
    a_counter = 0;
    for _i in 0..8192 {
        if unsafe { *virt_ptr } == ('a' as u8) {
            a_counter += 1;
        }
        virt_ptr = virt_ptr.wrapping_add(1);
    }

    if a_counter == 8192 {
        println!("Physical range mapping verified!");
    } else {
        println!(
            "Issue with physical range mapping: only found {} 'a' characters!",
            a_counter
        );
    }

    _ = sys::munmap(mmap_addr, 0).unwrap();

    sys::shutdown();
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
