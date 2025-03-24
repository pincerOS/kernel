use crate::event::async_handler::{run_async_handler, HandlerContext};
use crate::event::context::Context;

/// syscall get_time_ms() -> u64
pub unsafe fn sys_get_time_ms(ctx: &mut Context) -> *mut Context {
    ctx.regs[0] = crate::device::system_timer::get_time() as usize / 1000;
    ctx
}

/// syscall sleep_ms(time: u64)
pub unsafe fn sys_sleep_ms(ctx: &mut Context) -> *mut Context {
    let duration = ctx.regs[0].saturating_mul(1000) as u64;

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        // TODO: interruptible sleep?
        crate::sync::time::sleep(duration).await;
        context.regs().regs[0] = 0;
        context.resume_final()
    })
}
