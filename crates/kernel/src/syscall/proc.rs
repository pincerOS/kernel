use alloc::sync::Arc;

use crate::event::context::{deschedule_thread, Context, DescheduleAction, CORES};
use crate::{event, event::thread, shutdown};

pub unsafe fn sys_shutdown(_ctx: &mut Context) -> *mut Context {
    shutdown();
}

pub unsafe fn sys_exit(ctx: &mut Context) -> *mut Context {
    let thread = CORES.with_current(|core| core.thread.take());
    let mut thread = thread.expect("usermode syscall without active thread");
    unsafe { thread.save_context(ctx.into(), false) };
    unsafe { deschedule_thread(DescheduleAction::FreeThread, Some(thread)) }
}

pub unsafe fn sys_spawn(ctx: &mut Context) -> *mut Context {
    let user_entry = ctx.regs[0];
    let user_sp = ctx.regs[1];
    let user_x0 = ctx.regs[2];
    let flags = ctx.regs[3];

    let cur_process = CORES.with_current(|core| {
        let thread = core.thread.take().unwrap();
        // TODO: don't require cloning here
        // TODO: how to make longer periods of access to the current thread sound?
        // (ie. either internal mutability, or can't yield/preempt/check preempt status...)
        let cur_process = thread.process.clone();
        core.thread.set(Some(thread));
        cur_process
    });
    let old_process = cur_process.unwrap();

    let process;
    if flags == 1 {
        // Same process, shared memory
        process = old_process;
    } else {
        process = Arc::new(old_process.fork());
    }

    println!(
        "Creating new process with page dir {:#010}",
        process.get_ttbr0()
    );
    let mut user_thread = unsafe { thread::Thread::new_user(process, user_sp, user_entry) };
    user_thread.context.as_mut().unwrap().regs[0] = user_x0;
    event::SCHEDULER.add_task(event::Event::ScheduleThread(user_thread));

    ctx
}
