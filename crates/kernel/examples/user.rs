#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::*;

use core::arch::global_asm;

global_asm!(
    "
.global user_code_start, user_code_end
user_code_start:
    svc 2

.balign 4
user_code_end:
    udf #2
    "
);

#[allow(improper_ctypes)]
extern "C" {
    static user_code_start: ();
    static user_code_end: ();
}

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    // Create user region (mapped at 0x20_000 in virtual memory)
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
    unsafe { core::arch::asm!("msr TTBR0_EL1, {0}", "isb", in(reg) ttbr0) };

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
        let code_start = (&raw const user_code_start).cast::<u32>();
        let code_end = (&raw const user_code_end).cast::<u32>();
        let len = unsafe { code_end.offset_from(code_start) }
            .try_into()
            .unwrap();
        let code_src = unsafe { core::slice::from_raw_parts(code_start, len) };

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
