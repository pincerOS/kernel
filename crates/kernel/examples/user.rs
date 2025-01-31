#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use core::arch::asm;
use core::mem::MaybeUninit;
use core::ptr::copy_nonoverlapping;

use alloc::boxed::Box;
use kernel::*;

use context::{deschedule_thread, Context, DescheduleAction, CORES};

unsafe fn sys_shutdown(_ctx: &mut Context) -> *mut Context {
    shutdown();
}
unsafe fn sys_hello(ctx: &mut Context) -> *mut Context {
    println!("Hello world syscall");
    ctx
}
unsafe fn sys_yield(ctx: &mut Context) -> *mut Context {
    let (core_sp, thread) = CORES.with_current(|core| (core.core_sp.get(), core.thread.take()));
    let mut thread = thread.expect("usermode syscall without active thread");
    unsafe { thread.save_context(ctx.into()) };

    let action = DescheduleAction::Yield;
    unsafe { deschedule_thread(core_sp, thread, action) }
}
unsafe fn sys_print(ctx: &mut Context) -> *mut Context {
    let ptr = ctx.regs[0];
    let len = ctx.regs[1];
    // TODO: soundness (check user permissions for the range)
    let data = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    let mut stdout = device::uart::UART.get().lock();
    for c in data {
        stdout.writec(*c);
    }
    ctx
}
unsafe fn sys_exit(ctx: &mut Context) -> *mut Context {
    let (core_sp, thread) = CORES.with_current(|core| (core.core_sp.get(), core.thread.take()));
    let mut thread = thread.expect("usermode syscall without active thread");
    unsafe { thread.save_context(ctx.into()) };

    let action = DescheduleAction::FreeThread;
    unsafe { deschedule_thread(core_sp, thread, action) }
}
unsafe fn sys_spawn(ctx: &mut Context) -> *mut Context {
    let user_entry = ctx.regs[0];
    let user_sp = ctx.regs[1];
    let flags = ctx.regs[2];

    let cur_page_dir = CORES.with_current(|core| {
        let thread = core.thread.take().unwrap();
        let page_dir = thread.user_regs.as_ref().unwrap().ttbr0_el1;
        core.thread.set(Some(thread));
        page_dir
    });

    let page_dir;
    if flags == 1 {
        // shared mem
        page_dir = cur_page_dir;
    } else {
        // fork-style
        let (dst_data, new_page_dir) = crate::arch::memory::create_user_region();
        let dst_data = dst_data as *mut u8;
        // This is a massive hack
        let buf_size = 1 << 16;
        let mut buffer: Box<[MaybeUninit<u8>]> = Box::new_uninit_slice(buf_size);

        let buf_ptr = buffer.as_mut_ptr().cast();
        let src_data = 0x20_0000 as *const u8;
        let src_size = 0x20_0000 * 15;
        for i in 0..(src_size / buf_size) {
            unsafe {
                copy_nonoverlapping(src_data.byte_add(i * buf_size), buf_ptr, buf_size);
                asm!("msr TTBR0_EL1, {0}", "isb", in(reg) new_page_dir);
                copy_nonoverlapping(buf_ptr, dst_data.byte_add(i * buf_size), buf_size);
                asm!("msr TTBR0_EL1, {0}", "isb", in(reg) cur_page_dir);
            }
        }
        page_dir = cur_page_dir;
    }

    let user_thread = unsafe { thread::Thread::new_user(user_sp, user_entry, page_dir) };
    event::SCHEDULER.add_task(event::Event::ScheduleThread(user_thread));

    ctx
}

static INIT_CODE: &[u8] = kernel::util::include_bytes_align!(u32, "../../init/init.bin");

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    unsafe {
        exceptions::register_syscall_handler(1, sys_shutdown);
        exceptions::register_syscall_handler(2, sys_hello);
        exceptions::register_syscall_handler(3, sys_yield);
        exceptions::register_syscall_handler(4, sys_print);
        exceptions::register_syscall_handler(5, sys_spawn);
        exceptions::register_syscall_handler(6, sys_exit);
    }

    unsafe { crate::arch::memory::init_physical_alloc() };

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
    unsafe { asm!("msr TTBR0_EL1, {0}", "isb", in(reg) ttbr0) };

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

    let start = sync::get_time();
    {
        let code_src = bytemuck::cast_slice::<_, u32>(INIT_CODE);

        let user_code_ptr = user_region.cast::<u32>();
        let user_code = unsafe { core::slice::from_raw_parts_mut(user_code_ptr, code_src.len()) };
        user_code.copy_from_slice(code_src);
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
