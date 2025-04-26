use crate::event::async_handler::{run_async_handler, HandlerContext};
use crate::event::context::{deschedule_thread, Context, DescheduleAction, CORES};
use crate::process::ExitStatus;

//TODO: register these syscalls

//Users should be able to redefine these?

///Register a function to run when the user process encounters a page fault
pub unsafe fn sys_register_user_page_fault_handler(ctx: &mut Context) -> *mut Context {
    let user_page_fault_handler: fn() = unsafe { core::mem::transmute::<usize, fn()>(ctx.regs[0]) };
    //TODO: add safety checks on this pointer

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        proc.signal_handlers.user_page_fault_handler = Some(user_page_fault_handler);

        context.resume_return(0)
    })
}

///Register a function to run when the process receives a kill signal
pub unsafe fn sys_register_kill_block_handler(ctx: &mut Context) -> *mut Context {
    let user_kill_block_handler: fn() = unsafe { core::mem::transmute::<usize, fn()>(ctx.regs[0]) };
    //TODO: add safety checks on this pointer

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        proc.signal_handlers.kill_block_handler = Some(user_kill_block_handler);

        context.resume_return(0)
    })
}

///Kill which can be blocked if the user registers a handler
pub unsafe fn sys_kill(ctx: &mut Context) -> *mut Context {
    todo!();
}

///Kill which cannot be blocked
pub unsafe fn sys_kill_unblockable(ctx: &mut Context) -> *mut Context {
    todo!();
}

