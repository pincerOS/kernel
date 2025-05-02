use crate::event::async_handler::{run_async_handler, HandlerContext};
use crate::event::context::{deschedule_thread, Context, DescheduleAction, CORES};
use crate::process::signal::SignalCode;
use crate::process::{signal, ExitStatus};

//TODO: register these syscalls

//Users should be able to redefine these?

///Register a function to run when the user process encounters a page fault
pub unsafe fn sys_register_signal_handler(ctx: &mut Context) -> *mut Context {
    let signal_number= ctx.regs[0] as u32;
    let user_handler: fn() = unsafe { core::mem::transmute::<usize, fn()>(ctx.regs[1]) };
    //TODO: add safety checks on this pointer
    let mut ret_val = 0;
    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();
        let signal_code = SignalCode::from(signal_number);
        match signal_code {
            signal::SignalCode::PageFault => proc.signal_handlers.lock().user_page_fault_handler = Some(user_handler),
            signal::SignalCode::KillBlockable => proc.signal_handlers.lock().kill_block_handler = Some(user_handler),
            _ => ret_val = -1, //User is trying to register a handler for an unsupported signal
        } 

        context.resume_return(ret_val as usize)
    })
}

///Return from a signal handler
pub unsafe fn sys_sigreturn(ctx: &mut Context) -> *mut Context {

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();
        
        proc.signal_flags.set(signal::SignalFlagOptions::IN_HANDLER, false);
        //replacing context with backup context is done in event loop

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

