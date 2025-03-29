use alloc::sync::Arc;

use crate::event::async_handler::{run_async_handler, HandlerContext};
use crate::event::context::Context;
use crate::process::fd::{self, FileDescriptor};
use crate::sync::semaphore::Semaphore;

pub unsafe fn sys_sem_create(ctx: &mut Context) -> *mut Context {
    let value = ctx.regs[0];

    run_async_handler(ctx, async move |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let descriptor = SemFd(Semaphore::new(value as isize));
        let fd = proc.file_descriptors.lock().insert(Arc::new(descriptor));

        context.resume_return(fd)
    })
}

pub unsafe fn sys_sem_up(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];

    run_async_handler(ctx, async move |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let file = proc.file_descriptors.lock().get(fd).cloned();
        let Some(file) = file else {
            return context.resume_return(-1i64 as usize);
        };
        let Some(sem) = file.as_any().downcast_ref::<SemFd>() else {
            return context.resume_return(-1i64 as usize);
        };
        sem.0.up();
        context.resume_return(0)
    })
}

pub unsafe fn sys_sem_down(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];

    run_async_handler(ctx, async move |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let file = proc.file_descriptors.lock().get(fd).cloned();
        let Some(file) = file else {
            return context.resume_return(-1i64 as usize);
        };
        let Some(sem) = file.as_any().downcast_ref::<SemFd>() else {
            return context.resume_return(-1i64 as usize);
        };
        sem.0.down().await;
        context.resume_return(0)
    })
}

struct SemFd(Semaphore);

impl FileDescriptor for SemFd {
    fn is_same_file(&self, other: &dyn FileDescriptor) -> bool {
        let other = other.as_any().downcast_ref::<Self>();
        other.map(|o| core::ptr::eq(&self, &o)).unwrap_or(false)
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
