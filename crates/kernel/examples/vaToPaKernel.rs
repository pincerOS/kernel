#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use alloc::boxed::Box;
use kernel::*;

const VIRTUAL_ADDR: usize = 0xFFFF_FFFF_FE00_0000 | 0x1E00000;
static HELLO_CHARS: [u8; 5] = *b"hello";

#[repr(C, align(4096))]
struct SomePage([u8; 4096]);

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");
    println!("Staring basic vmm test");

    unsafe {
        crate::arch::memory::init_physical_alloc();
        crate::arch::memory::init();
    }
    println!("Init complete");

    //hack: get a "page" from the heap and try to map it to some virtual address
    let page_box = Box::new(SomePage([0; 4096]));
    let page_ptr: *mut SomePage = Box::into_raw(page_box);

    //Copying hello into the first few bytes of the page
    let phys_addr: usize = crate::arch::memory::physical_addr((page_ptr).addr()).unwrap() as usize;

    unsafe {
        core::ptr::copy(
            &raw const HELLO_CHARS,
            page_ptr as *mut [u8; 5],
            HELLO_CHARS.len(),
        );
    }

    println!(
        "Attempting to map pa {:x} to va: {:x}",
        phys_addr, VIRTUAL_ADDR
    );
    unsafe {
        match crate::arch::memory::map_pa_to_va_kernel(phys_addr, VIRTUAL_ADDR) {
            Ok(()) => println!("Done mapping!"),
            Err(e) => println!("Error: {}", e),
        }
    }

    let virt_ptr: *const u8 = VIRTUAL_ADDR as *const u8;
    let mut all_good = true;

    for i in 0..5 {
        if unsafe { *(virt_ptr.wrapping_add(i)) != HELLO_CHARS[i] } {
            all_good = false;
            break;
        }
    }

    if all_good {
        println!("Physical to virtual mappping success");
    } else {
        println!("Physical to virtual mapping encountered an error!");
    }

    println!("End of basic vmm test");
}
