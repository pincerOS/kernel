use alloc::boxed::Box;
use alloc::sync::Arc;

use crate::event::async_handler::{run_async_handler, run_event_handler, HandlerContext};
use crate::event::context::{deschedule_thread, Context, DescheduleAction};
use crate::event::thread::Thread;
use crate::process::fd::{self, FileDescriptor};
use crate::process::ExitStatus;
use crate::sync::once_cell::BlockingOnceCell;
use crate::{event, shutdown};

pub unsafe fn sys_shutdown(ctx: &mut Context) -> *mut Context {
    run_event_handler(ctx, move |_context: HandlerContext<'_>| {
        shutdown();
    })
}

pub unsafe fn exit_exception(mut thread: Box<Thread>, ctx: &mut Context, status: u32) -> ! {
    let exit_code = &thread.process.as_ref().unwrap().exit_code;
    exit_code.set(crate::process::ExitStatus {
        status: status as u32,
    });
    unsafe { thread.save_context(ctx.into(), false) };
    unsafe { deschedule_thread(DescheduleAction::FreeThread, Some(thread)) }
}

pub unsafe fn sys_exit(ctx: &mut Context) -> *mut Context {
    let status = ctx.regs[0];

    run_event_handler(ctx, move |context: HandlerContext<'_>| {
        let thread = context.detach_thread();

        // TODO: split exit into process exit and thread exit?
        // TODO: ensure processes can't exit without setting this
        let exit_code = &thread.process.as_ref().unwrap().exit_code;
        exit_code.set(crate::process::ExitStatus {
            status: status as u32,
        });

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
            wait_fd = (-1isize) as usize;
        } else {
            process = Arc::new(old_process.fork().await);
            let descriptor = WaitFd(process.exit_code.clone());
            let fd = old_process
                .file_descriptors
                .lock()
                .insert(Arc::new(descriptor));
            wait_fd = fd;
        }

        println!(
            "Creating new process with page dir {:#010x}",
            process.get_ttbr0()
        );
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

struct WaitFd(Arc<BlockingOnceCell<ExitStatus>>);

impl FileDescriptor for WaitFd {
    fn is_same_file(&self, other: &dyn FileDescriptor) -> bool {
        let other = other.as_any().downcast_ref::<Self>();
        other.map(|o| Arc::ptr_eq(&self.0, &o.0)).unwrap_or(true)
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
