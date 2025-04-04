#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use crate::arch::memory::{map_pa_to_va, UnifiedTranslationTable};
use crate::event::context;
use crate::event::thread;
use alloc::boxed::Box;
use core::arch::asm;
use kernel::*;

//Current version of create_user_table leaves this addr free
const VIRTUAL_ADDR: usize = 0x1E00000;
static HELLO_CHARS: [u8; 5] = *b"hello";
static WORLD_CHARS: [u8; 5] = *b"world";

#[repr(C, align(4096))]
struct SomePage([u8; 4096]);

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    let mut process = crate::process::Process::new();
    // Assume fixed mapped range in user process (0x20_0000 in virtual memory)
    // TODO: mmap instead
    let user_region = 0x20_0000 as *mut u8;
    let ttbr0 = process.get_ttbr0();

    // Mark current thread as using TTBR0, so that preemption saves
    // and restores the register.
    context::CORES.with_current(|core| {
        let mut thread = core.thread.take().unwrap();
        thread.user_regs = Some(thread::UserRegs {
            ttbr0_el1: ttbr0,
            usermode: false,
        });
        core.thread.set(Some(thread));
    });
    // Enable the user-mode address space in this thread
    unsafe {
        asm!("msr TTBR0_EL1, {0}", "isb", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) ttbr0)
    };

    println!("User ptr: {:p}", user_region);
    // TODO: sometimes get an insn abort here? (leads to UART deadlock)
    println!(
        "Physical addr: {:?}",
        memory::physical_addr(user_region.addr())
    );
    let access = crate::arch::memory::at_s1e0r(user_region.addr());
    println!(
        "user access: {:?}",
        access.map(|b| b.bits()).map_err(|e| e.bits())
    );

    //hack: get a "page" from the heap and try to map it to some virtual address
    let page_box = Box::new(SomePage([0; 4096]));
    let page_ptr: *mut SomePage = Box::into_raw(page_box);
    println!("page_ptr: {:x}", page_ptr as usize);
    //Copying hello into the first few bytes of the page
    let phys_addr: usize = crate::arch::memory::physical_addr((page_ptr).addr()).unwrap() as usize;

    unsafe {
        core::ptr::copy_nonoverlapping(
            &raw const HELLO_CHARS[0],
            page_ptr as *mut u8,
            HELLO_CHARS.len(),
        );
    }

    println!(
        "Attempting to map pa {:x} to va: {:x}",
        phys_addr, VIRTUAL_ADDR
    );
    unsafe {
        let user_translation_table: *mut UnifiedTranslationTable = &mut *process.page_table.table;

        match map_pa_to_va(phys_addr, VIRTUAL_ADDR, user_translation_table, false, true) {
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
        println!("Passed first check");
    } else {
        println!("First check failed");
    }

    all_good = true;

    unsafe {
        core::ptr::copy_nonoverlapping(
            &raw const WORLD_CHARS[0],
            VIRTUAL_ADDR as *mut u8,
            WORLD_CHARS.len(),
        );
    }

    for i in 0..5 {
        if unsafe { (*page_ptr).0[i] != WORLD_CHARS[i] } {
            all_good = false;
            break;
        }
    }

    if all_good {
        println!("Passed second check");
    } else {
        println!("Second check failed");
    }

    println!("Done with basic pa to va user mapping test!");
}
