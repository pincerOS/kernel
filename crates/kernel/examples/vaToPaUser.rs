#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use core::arch::asm;

use kernel::*;
use alloc::boxed::Box;

//Current version of create_user_table leaves this addr free
const VIRTUAL_ADDR: usize = 0x1E00000;
static HELLO_CHARS: [u8; 5] = *b"hello";
static WORLD_CHARS: [u8; 5] = *b"world";

#[repr(C, align(4096))]
struct SomePage([u8; 4096]);

static INIT_CODE: &[u8] = kernel::util::include_bytes_align!(u32, "../../init/init.bin");

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    unsafe { syscall::register_syscalls() };
    unsafe { crate::arch::memory::init_physical_alloc() };
    
    unsafe { crate::arch::memory::init_page_allocator()  };

    let (_stdio, mut stdin_tx, mut stdout_rx) = {
        let (stdin_tx, stdin_rx) = ringbuffer::channel();
        let (stdout_tx, stdout_rx) = ringbuffer::channel();
        let stdio_chan = syscall::channel::alloc_obj(syscall::channel::Object::Channel {
            send: stdout_tx,
            recv: stdin_rx,
        });
        (stdio_chan, stdin_tx, stdout_rx)
    };

    task::spawn_async(async move {
        let mut buf = [0; 256];
        let mut buf_len = 0;
        loop {
            {
                let uart = device::uart::UART.get();
                let mut guard = uart.lock();
                while let Some(c) = guard.try_getc() {
                    buf[buf_len] = c;
                    buf_len += 1;
                    if buf_len >= 256 {
                        break;
                    }
                }
            }
            if buf_len > 0 {
                let msg = syscall::channel::Message {
                    tag: 0,
                    objects: [const { None }; 4],
                    data: Some(buf[..buf_len].into()),
                };
                stdin_tx.send_async(msg).await;
                buf_len = 0;
            }
            task::yield_future().await;
        }
    });
    task::spawn_async(async move {
        loop {
            let input = stdout_rx.recv_async().await;
            if let Some(data) = input.data {
                let uart = device::uart::UART.get();
                let mut stdout = uart.lock();
                for c in data {
                    stdout.writec(c);
                }
            }
        }
    });

    // Create user region (mapped at 0x20_0000 in virtual memory)
    let (user_region, ttbr0) = unsafe { crate::arch::memory::create_user_region() };

    // Mark current thread as using TTBR0, so that preemption saves
    // and restores the register.
    context::CORES.with_current(|core| {
        let mut thread = core.thread.take().unwrap();
        thread.user_regs = Some(thread::UserRegs {
            sp_el0: 0,
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

    let user_translation_table_ptr = unsafe { crate::arch::memory::get_prev_user_translation_table_va()  }as *mut crate::arch::memory::UserTranslationTable; 
    
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
        match crate::arch::memory::map_pa_to_va_user(phys_addr, VIRTUAL_ADDR, &mut (*user_translation_table_ptr)) {
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

    let start = sync::get_time();
    unsafe {
        core::ptr::copy_nonoverlapping(
            INIT_CODE.as_ptr(),
            user_region.cast::<u8>(),
            INIT_CODE.len(),
        );
    }
    let end = sync::get_time();

    // TODO: this sometimes takes significantly longer?
    // "Done copying user data, took 868749µs"
    println!("Done copying user data, took {:4}µs", end - start);

    let user_sp = 0x100_0000;
    let user_entry = 0x20_0000;

    let user_thread = unsafe { thread::Thread::new_user(user_sp, user_entry, ttbr0) };

    event::SCHEDULER.add_task(event::Event::ScheduleThread(user_thread));

    thread::stop();
}
