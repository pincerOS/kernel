use crate::event::async_handler::{run_async_handler, HandlerContext};
use crate::event::context::Context;

use super::current_process;

bitflags::bitflags! {
    struct DupFlags: u32 {
    }
}

/// syscall dup3(old_fd: u32, new_fd: u32, flags: DupFlags) -> i64
pub unsafe fn sys_dup3(ctx: &mut Context) -> *mut Context {
    let old_fd = ctx.regs[0];
    let new_fd = ctx.regs[1];
    let flags = ctx.regs[2];

    let Some(_flags) = u32::try_from(flags).ok().and_then(DupFlags::from_bits) else {
        ctx.regs[0] = i64::from(-1) as usize;
        return ctx;
    };

    let proc = current_process().unwrap();

    let mut guard = proc.file_descriptors.lock();
    let Some(old) = guard.get(old_fd).cloned() else {
        ctx.regs[0] = i64::from(-1) as usize;
        return ctx;
    };

    let to_close = guard.set(new_fd, old);

    if let Some(desc) = to_close {
        // TODO: we should be careful about where/when fd destructors are run
        drop(desc);
    }

    ctx.regs[0] = new_fd;
    ctx
}

/// syscall close(fd: u32) -> i64
pub unsafe fn sys_close(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];
    let proc = current_process().unwrap();

    let mut guard = proc.file_descriptors.lock();
    if let Some(desc) = guard.close(fd) {
        // TODO: we should be careful about where/when fd destructors are run
        drop(desc);

        ctx.regs[0] = i64::from(0) as usize;
        ctx
    } else {
        ctx.regs[0] = i64::from(-1) as usize;
        ctx
    }
}

/// syscall pread(fd: u32, buf: *mut u8, len: u64, offset: u64) -> i64
pub unsafe fn sys_pread(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];
    let buf_ptr = ctx.regs[1];
    let buf_len = ctx.regs[2];
    let offset = ctx.regs[3];

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let file = proc.file_descriptors.lock().get(fd).cloned();
        let Some(file) = file else {
            context.regs().regs[0] = i64::from(-1) as usize;
            return context.resume_final();
        };

        // TODO: sound abstraction for usermode buffers...
        // (prevent TOCTOU issues, pin pages to prevent user unmapping them,
        // deal with unmapped pages...)
        // TODO: check user buffers
        let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };

        let res = file.read(offset as u64, buf).await;

        context.regs().regs[0] = res.0 as usize;
        context.resume_final()
    })
}

/// syscall pwrite(fd: u32, buf: *const u8, len: u64, offset: u64) -> i64
pub unsafe fn sys_pwrite(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];
    let buf_ptr = ctx.regs[1];
    let buf_len = ctx.regs[2];
    let offset = ctx.regs[3];

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let file = proc.file_descriptors.lock().get(fd).cloned();
        let Some(file) = file else {
            context.regs().regs[0] = i64::from(-1) as usize;
            return context.resume_final();
        };

        // TODO: sound abstraction for usermode buffers...
        // (prevent TOCTOU issues, pin pages to prevent user unmapping them,
        // deal with unmapped pages...)
        // TODO: check user buffers
        let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, buf_len) };

        let res = file.write(offset as u64, buf).await;

        context.regs().regs[0] = res.0 as usize;
        context.resume_final()
    })
}
