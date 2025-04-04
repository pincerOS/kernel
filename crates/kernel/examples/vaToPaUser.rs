#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use crate::arch::memory::{map_va_to_pa, UnifiedTranslationTable};
use crate::event::context;
use crate::event::thread;
use alloc::boxed::Box;
use core::arch::asm;
use kernel::*;
use alloc::alloc::{alloc, Layout};

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

    let user_translation_table: *mut UnifiedTranslationTable = &mut *process.page_table.table;
    
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
        match map_va_to_pa(phys_addr, VIRTUAL_ADDR, user_translation_table, false, true) {
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

    //Huge page test
    let huge_page_virt_addr: usize = (VIRTUAL_ADDR + 4096).next_multiple_of(0x20_0000);
    let huge_page_layout: Layout = Layout::from_size_align(0x20_0000, 0x20_0000).unwrap();
    let huge_page_ptr: *mut u8 = unsafe { alloc(huge_page_layout) };
    unsafe { core::ptr::write_bytes(huge_page_ptr, 'z' as u8, 0x20_0000); }
    
    let huge_phys_addr: usize = crate::arch::memory::physical_addr((huge_page_ptr).addr()).unwrap() as usize;
    println!("huge page physical addr: {:x} huge page virtual addr in box: {:x}", huge_phys_addr, huge_page_ptr as usize);

    println!(
        "Attempting to map pa {:x} to va: {:x}",
        huge_phys_addr, huge_page_virt_addr
    );
    unsafe {
        match map_va_to_pa(huge_phys_addr, huge_page_virt_addr, user_translation_table, true, true) {
            Ok(()) => println!("Done mapping!"),
            Err(e) => println!("Error: {}", e),
        }
    }

    let mut huge_virt_ptr: *const u8 = huge_page_virt_addr as *const u8;
    let mut z_counter = 0;
    
    for _i in 0..0x20_0000 {
        if unsafe { *huge_virt_ptr } == 'z' as u8 {
            z_counter += 1;
            //println!("First correct addr: {:x}", huge_virt_ptr as usize);
            //break;
        } else {
            //println!("First wrong addr: {:x}", huge_virt_ptr as usize);
            //break;
        }

        huge_virt_ptr = huge_virt_ptr.wrapping_add(1);
    }

    if z_counter == 0x20_0000 {
        println!("Passed huge page mapping test");
    } else {
        println!("Failed huge page mapping test. Expected 0x20_0000 z characters, found {:x} z characters", z_counter);
    }

    println!("Done with basic pa to va user mapping test!");
}

