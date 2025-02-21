use crate::event::context::{deschedule_thread, Context, DescheduleAction, CORES};

pub unsafe fn sys_yield(ctx: &mut Context) -> *mut Context {
    let thread = CORES.with_current(|core| core.thread.take());
    let mut thread = thread.expect("usermode syscall without active thread");
    unsafe { thread.save_context(ctx.into()) };
    unsafe { deschedule_thread(DescheduleAction::Yield, Some(thread)) }
}
