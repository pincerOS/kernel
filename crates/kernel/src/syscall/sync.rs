use crate::event::async_handler::{run_event_handler, HandlerContext};
use crate::event::context::{deschedule_thread, Context, DescheduleAction};

pub unsafe fn sys_yield(ctx: &mut Context) -> *mut Context {
    run_event_handler(ctx, move |context: HandlerContext<'_>| {
        let thread = context.detach_thread();
        unsafe { deschedule_thread(DescheduleAction::Yield, Some(thread)) }
    })
}
