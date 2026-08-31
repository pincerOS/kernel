use alloc::sync::Arc;

use crate::event::context::Context;
use crate::process::fd;
use crate::ringbuffer::channel;
use crate::sync::SpinLock;

use super::current_process;

bitflags::bitflags! {
    struct PipeFlags: u32 {
    }
}

/// syscall pipe(flags: PipeFlags) -> i64 | (u64, u64)
pub unsafe fn sys_pipe(ctx: &mut Context) -> *mut Context {
    let flags = ctx.regs[0];

    let Some(_flags) = u32::try_from(flags).ok().and_then(PipeFlags::from_bits) else {
        ctx.regs[0] = i64::from(-1) as usize;
        return ctx;
    };

    let proc = current_process().unwrap();

    let (tx, rx) = channel();
    let tx_fd = Arc::new(PipeWriteFd(SpinLock::new(tx)));
    let rx_fd = Arc::new(PipeReadFd(SpinLock::new(rx)));

    let mut guard = proc.file_descriptors.lock();
    let rx_fdi = guard.insert(rx_fd);
    let tx_fdi = guard.insert(tx_fd);

    ctx.regs[0] = rx_fdi;
    ctx.regs[1] = tx_fdi;
    ctx
}

const PIPE_SIZE: usize = 4096;

// TODO: byte-based pipe instead?
// TODO: unbounded?
// TODO: needs to be MPMC (threads, shared fds)
pub struct PipeReadFd(SpinLock<crate::ringbuffer::Receiver<PIPE_SIZE, u8>>);

pub struct PipeWriteFd(SpinLock<crate::ringbuffer::Sender<PIPE_SIZE, u8>>);

// TODO: how to handle non-zero offsets for non-seekable files?
impl fd::FileDescriptor for PipeWriteFd {
    fn is_same_file(&self, other: &dyn fd::FileDescriptor) -> bool {
        let Some(other) = other.as_any().downcast_ref::<Self>() else {
            return false;
        };
        core::ptr::eq(self, other)
    }
    fn kind(&self) -> fd::FileKind {
        fd::FileKind::Other
    }
    fn read<'a>(
        &'a self,
        _offset: u64,
        _buf: &'a mut [u8],
    ) -> fd::SmallFuture<'a, fd::FileDescResult> {
        fd::boxed_future(async move { fd::FileDescResult::err(1) })
    }
    fn write<'a>(&'a self, _offset: u64, buf: &'a [u8]) -> fd::SmallFuture<'a, fd::FileDescResult> {
        if buf.is_empty() {
            return fd::boxed_future(async move { fd::FileDescResult::ok(0) });
        }
        fd::boxed_future(async move {
            let v = buf[0];
            self.0.lock().send(v).await;
            fd::FileDescResult::ok(1)
        })
    }
    fn size<'a>(&'a self) -> fd::SmallFuture<'a, fd::FileDescResult> {
        fd::boxed_future(async move { Ok(0u64).into() })
    }
    fn mmap_page(&self, _offset: u64) -> fd::SmallFuture<Option<fd::FileDescResult>> {
        fd::boxed_future(async move { None })
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

// TODO: how to handle non-zero offsets for non-seekable files?
impl fd::FileDescriptor for PipeReadFd {
    fn is_same_file(&self, other: &dyn fd::FileDescriptor) -> bool {
        let Some(other) = other.as_any().downcast_ref::<Self>() else {
            return false;
        };
        core::ptr::eq(self, other)
    }
    fn kind(&self) -> fd::FileKind {
        fd::FileKind::Other
    }
    fn read<'a>(
        &'a self,
        _offset: u64,
        buf: &'a mut [u8],
    ) -> fd::SmallFuture<'a, fd::FileDescResult> {
        if buf.is_empty() {
            return fd::boxed_future(async move { fd::FileDescResult::ok(0) });
        }
        fd::boxed_future(async move {
            let c = self.0.lock().recv().await;
            buf[0] = c;
            fd::FileDescResult::ok(1)
        })
    }
    fn write<'a>(
        &'a self,
        _offset: u64,
        _buf: &'a [u8],
    ) -> fd::SmallFuture<'a, fd::FileDescResult> {
        fd::boxed_future(async move { fd::FileDescResult::err(1) })
    }
    fn size<'a>(&'a self) -> fd::SmallFuture<'a, fd::FileDescResult> {
        fd::boxed_future(async move { Ok(0u64).into() })
    }
    fn mmap_page(&self, _offset: u64) -> fd::SmallFuture<Option<fd::FileDescResult>> {
        fd::boxed_future(async move { None })
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
