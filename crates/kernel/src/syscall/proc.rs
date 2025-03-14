use alloc::boxed::Box;
use core::arch::asm;
use core::mem::MaybeUninit;
use core::ptr::copy_nonoverlapping;

use crate::event::context::{deschedule_thread, Context, DescheduleAction, CORES};
use crate::{event, event::thread, shutdown};

pub unsafe fn sys_shutdown(_ctx: &mut Context) -> *mut Context {
    shutdown();
}

pub unsafe fn sys_exit(ctx: &mut Context) -> *mut Context {
    let thread = CORES.with_current(|core| core.thread.take());
    let mut thread = thread.expect("usermode syscall without active thread");
    unsafe { thread.save_context(ctx.into()) };
    unsafe { deschedule_thread(DescheduleAction::FreeThread, Some(thread)) }
}

pub unsafe fn sys_spawn(ctx: &mut Context) -> *mut Context {
    let user_entry = ctx.regs[0];
    let user_sp = ctx.regs[1];
    let user_x0 = ctx.regs[2];
    let flags = ctx.regs[3];

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
        // This is a massive hack
        let buf_size = 1 << 16;
        let mut buffer: Box<[MaybeUninit<u8>]> = Box::new_uninit_slice(buf_size);
        let buf_ptr: *mut u8 = buffer.as_mut_ptr().cast();

        // fork-style
        let (dst_data, new_page_dir) = unsafe { crate::arch::memory::create_user_region() };
        let dst_data = dst_data as *mut u8;
        let src_data = 0x20_0000 as *const u8;
        let src_size = 0x20_0000 * 7;

        assert_eq!(src_data, dst_data);
        assert_ne!(cur_page_dir, new_page_dir);

        for i in 0..(src_size / buf_size) {
            // // This causes an ICE?
            // core::ptr::write_volatile(buf_ptr.cast::<[u8; 1 << 16]>(), core::ptr::read_volatile(src_data.byte_add(i * buf_size).cast::<[u8; 1 << 16]>()));
            // asm!("msr TTBR0_EL1, {0}", "isb", in(reg) new_page_dir);
            // core::ptr::write_volatile(dst_data.byte_add(i * buf_size).cast::<[u8; 1 << 16]>(), core::ptr::read_volatile(buf_ptr.cast::<[u8; 1 << 16]>()));
            // asm!("msr TTBR0_EL1, {0}", "isb", in(reg) cur_page_dir);

            unsafe {
                copy_nonoverlapping(src_data.byte_add(i * buf_size), buf_ptr, buf_size);
                asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) new_page_dir);
                copy_nonoverlapping(buf_ptr, dst_data.byte_add(i * buf_size), buf_size);
                asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) cur_page_dir);
            }
        }

        page_dir = new_page_dir;
    }

    println!("Creating new process with page dir {:#010}", page_dir);
    let mut user_thread = unsafe { thread::Thread::new_user(user_sp, user_entry, page_dir) };
    user_thread.context.as_mut().unwrap().regs[0] = user_x0;
    event::SCHEDULER.add_task(event::Event::ScheduleThread(user_thread));

    ctx
}
