use crate::event::context::{deschedule_thread, Context, DescheduleAction, CORES};

pub unsafe fn sys_yield(ctx: &mut Context) -> *mut Context {
    let (core_sp, thread) = CORES.with_current(|core| (core.core_sp.get(), core.thread.take()));
    let mut thread = thread.expect("usermode syscall without active thread");
    unsafe { thread.save_context(ctx.into()) };

    let action = DescheduleAction::Yield;
    unsafe { deschedule_thread(core_sp, Some(thread), action) }
}
