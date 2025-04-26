use alloc::boxed::Box;
use alloc::sync::Arc;

use crate::event::async_handler::{run_async_handler, run_event_handler, HandlerContext};
use crate::event::context::{deschedule_thread, Context, DescheduleAction, CORES};
use crate::event::thread::Thread;
use crate::process::fd::{self, FileDescriptor};
use crate::process::ExitStatus;
use crate::sync::once_cell::BlockingOnceCell;
use crate::{event, shutdown};

const SHUTDOWN_ENABLED: bool = false;

pub unsafe fn sys_shutdown(ctx: &mut Context) -> *mut Context {
    run_event_handler(ctx, move |context: HandlerContext<'_>| {
        if SHUTDOWN_ENABLED {
            shutdown();
        } else {
            println!("Not shutting down...");
            let thread = context.detach_thread();
            let exit_code = &thread.process.as_ref().unwrap().exit_code;
            exit_code
                .try_set(crate::process::ExitStatus { status: 0 as u32 })
                .ok();
            unsafe { deschedule_thread(DescheduleAction::FreeThread, Some(thread)) }
        }
    })
}

pub unsafe fn exit_current_user_thread(ctx: &mut Context, status: u32) -> ! {
    let thread = CORES.with_current(|core| core.thread.take());
    let mut thread = thread.expect("usermode syscall without active thread");

    thread.last_context = core::ptr::NonNull::from(&mut *ctx);
    unsafe { thread.save_user_regs() };
    unsafe { thread.save_context(ctx.into(), false) };

    unsafe { crate::sync::enable_interrupts() };
    unsafe { exit_user_thread(thread, status) }
}

pub unsafe fn exit_user_thread(mut thread: Box<Thread>, status: u32) -> ! {
    thread.set_exited(status);
    unsafe { deschedule_thread(DescheduleAction::FreeThread, Some(thread)) }
}

pub unsafe fn sys_exit(ctx: &mut Context) -> *mut Context {
    let status = ctx.regs[0];

    run_event_handler(ctx, move |context: HandlerContext<'_>| {
        let thread = context.detach_thread();

        // TODO: split exit into process exit and thread exit?
        // TODO: ensure processes can't exit without setting this
        let exit_code = &thread.process.as_ref().unwrap().exit_code;
        exit_code
            .try_set(crate::process::ExitStatus {
                status: status as u32,
            })
            .ok();

        unsafe { deschedule_thread(DescheduleAction::FreeThread, Some(thread)) }
    })
}

pub unsafe fn sys_spawn(ctx: &mut Context) -> *mut Context {
    let user_entry = ctx.regs[0];
    let user_sp = ctx.regs[1];
    let user_x0 = ctx.regs[2];
    let flags = ctx.regs[3];

    run_async_handler(ctx, async move |context: HandlerContext<'_>| {
        let old_process = context.cur_process().unwrap();

        let wait_fd;
        let process;

        if flags == 1 {
            // Same process, shared memory
            process = old_process.clone();
            wait_fd = i32::MAX as usize;
        } else {
            process = Arc::new(old_process.fork().await);
            let descriptor = WaitFd(process.exit_code.clone());
            let fd = old_process
                .file_descriptors
                .lock()
                .insert(Arc::new(descriptor));
            wait_fd = fd;
        }

        // println!(
        //     "Creating new process with page dir {:#010x}, initial sp {user_sp:#x}, entry {user_entry:#x}",
        //     process.get_ttbr0()
        // );
        let mut user_thread = unsafe { Thread::new_user(process, user_sp, user_entry) };
        user_thread.context.as_mut().unwrap().regs[0] = user_x0;
        event::SCHEDULER.add_task(event::Event::schedule_thread(user_thread));

        context.resume_return(wait_fd)
    })
}

/// syscall wait(fd: u32) -> i64
pub unsafe fn sys_wait(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];

    run_async_handler(ctx, async move |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let file = proc.file_descriptors.lock().get(fd).cloned();
        let Some(file) = file else {
            return context.resume_return(-1i64 as usize);
        };
        let Some(file) = file.as_any().downcast_ref::<WaitFd>() else {
            return context.resume_return(-1i64 as usize);
        };

        let status = file.0.get().await;

        context.resume_return(status.status as usize)
    })
}

/// syscall try_wait(fd: u32) -> i64
pub unsafe fn sys_try_wait(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];

    run_async_handler(ctx, async move |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let file = proc.file_descriptors.lock().get(fd).cloned();
        let Some(file) = file else {
            return context.resume_return(-1i64 as usize);
        };
        let Some(file) = file.as_any().downcast_ref::<WaitFd>() else {
            return context.resume_return(-1i64 as usize);
        };

        if let Some(status) = file.0.try_get() {
            context.resume_return(status.status as usize)
        } else {
            context.resume_return(i64::MIN as usize)
        }
    })
}

struct WaitFd(Arc<BlockingOnceCell<ExitStatus>>);

impl FileDescriptor for WaitFd {
    fn is_same_file(&self, other: &dyn FileDescriptor) -> bool {
        let other = other.as_any().downcast_ref::<Self>();
        other.map(|o| Arc::ptr_eq(&self.0, &o.0)).unwrap_or(false)
    }
    fn kind(&self) -> fd::FileKind {
        fd::FileKind::Other
    }
    fn read<'a>(
        &'a self,
        _offset: u64,
        _buf: &'a mut [u8],
    ) -> fd::SmallFuture<'a, fd::FileDescResult> {
        fd::boxed_future(async move { Err(1).into() })
    }
    fn write<'a>(
        &'a self,
        _offset: u64,
        _buf: &'a [u8],
    ) -> fd::SmallFuture<'a, fd::FileDescResult> {
        fd::boxed_future(async move { Err(1).into() })
    }
    fn size<'a>(&'a self) -> fd::SmallFuture<'a, fd::FileDescResult> {
        fd::boxed_future(async move { Err(1).into() })
    }
    fn mmap_page(&self, _offset: u64) -> fd::SmallFuture<Option<fd::FileDescResult>> {
        fd::boxed_future(async move { None })
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
