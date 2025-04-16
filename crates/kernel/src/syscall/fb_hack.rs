use alloc::collections::btree_map::BTreeMap;
use alloc::sync::Arc;

use crate::arch::memory::palloc::{PhysicalPage, Size4KiB, PAGE_ALLOCATOR};
use crate::arch::memory::vmm::PAGE_SIZE;
use crate::device::mailbox::RawFB;
use crate::device::MAILBOX;
use crate::event::async_handler::{run_async_handler, run_event_handler, HandlerContext};
use crate::event::context::Context;
use crate::process::fd;
use crate::sync::SpinLock;

/// syscall sys_alloc_fb(width: usize, height: usize) -> (fd, buffer_size: usize, width: usize, height: usize, pitch: usize)
pub unsafe fn sys_acquire_fb(ctx: &mut Context) -> *mut Context {
    let width = ctx.regs[0];
    let height = ctx.regs[1];

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        println!("| acquiring framebuffer");
        let fb = unsafe { MAILBOX.get().lock().get_framebuffer_raw(width, height) };

        let descriptor = FramebufferFd(fb);
        let fd = proc.file_descriptors.lock().insert(Arc::new(descriptor));

        {
            let mut regs = context.regs();
            regs.regs[0] = fd;
            regs.regs[1] = fb.size;
            regs.regs[2] = fb.width;
            regs.regs[3] = fb.height;
            regs.regs[4] = fb.pitch;
        }

        context.resume_final()
    })
}

struct FramebufferFd(RawFB);

impl fd::FileDescriptor for FramebufferFd {
    fn is_same_file(&self, other: &dyn fd::FileDescriptor) -> bool {
        let other = other.as_any().downcast_ref::<Self>();
        other.map(|o| core::ptr::eq(self, o)).unwrap_or(false)
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
    fn mmap_page(&self, offset: u64) -> fd::SmallFuture<Option<fd::FileDescResult>> {
        println!("Inside of memfd mmap page");
        let paddr_base = self.0.paddr;
        assert!(offset % PAGE_SIZE as u64 == 0);
        if offset >= self.0.size as u64 {
            fd::boxed_future(async move { None })
        } else {
            let page_addr = paddr_base + offset as usize;
            fd::boxed_future(async move { Some(fd::FileDescResult::ok(page_addr as u64)) })
        }
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

pub unsafe fn sys_poll_key_event(ctx: &mut Context) -> *mut Context {
    run_event_handler(ctx, |context: HandlerContext<'_>| {
        let res = crate::device::usb::keyboard::KEY_EVENTS.poll();
        let code = match res {
            Some(event) => event.code as usize | ((event.pressed as usize) << 8),
            None => (-1isize) as usize,
        };
        context.resume_return(code)
    })
}

pub unsafe fn sys_memfd_create(ctx: &mut Context) -> *mut Context {
    run_async_handler(ctx, async move |context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();
        let fd = Arc::new(MemFd::new());
        let fd = proc.file_descriptors.lock().insert(fd);
        context.resume_return(fd)
    })
}

struct MemFd {
    pages: SpinLock<BTreeMap<usize, PhysicalPage<Size4KiB>>>,
}

impl MemFd {
    fn new() -> Self {
        println!("Creating new mem fd");
        Self {
            pages: SpinLock::new(BTreeMap::new()),
        }
    }
}

impl Drop for MemFd {
    fn drop(&mut self) {
        let alloc = PAGE_ALLOCATOR.get();
        let pages = core::mem::take(&mut *self.pages.lock());
        for (_, page) in pages {
            alloc.dealloc_frame(page);
        }
    }
}

impl fd::FileDescriptor for MemFd {
    fn is_same_file(&self, other: &dyn fd::FileDescriptor) -> bool {
        let other = other.as_any().downcast_ref::<Self>();
        other.map(|o| core::ptr::eq(self, o)).unwrap_or(false)
    }
    fn kind(&self) -> fd::FileKind {
        fd::FileKind::Regular
    }
    fn read<'a>(
        &'a self,
        _offset: u64,
        _buf: &'a mut [u8],
    ) -> fd::SmallFuture<'a, fd::FileDescResult> {
        // TODO: impl read
        fd::boxed_future(async move { Err(1).into() })
    }
    fn write<'a>(
        &'a self,
        _offset: u64,
        _buf: &'a [u8],
    ) -> fd::SmallFuture<'a, fd::FileDescResult> {
        // TODO: impl write
        fd::boxed_future(async move { Err(1).into() })
    }
    fn size<'a>(&'a self) -> fd::SmallFuture<'a, fd::FileDescResult> {
        // TODO: Is size well defined for memfd?
        fd::boxed_future(async move { Err(1).into() })
    }
    fn mmap_page(&self, offset: u64) -> fd::SmallFuture<Option<fd::FileDescResult>> {
        assert!(offset % PAGE_SIZE as u64 == 0);

        let page_addr = {
            let mut pages = self.pages.lock();
            let frame = pages
                .entry(offset as usize)
                .or_insert_with(|| PAGE_ALLOCATOR.get().alloc_frame());
            frame.paddr
        };

        fd::boxed_future(async move { Some(fd::FileDescResult::ok(page_addr as u64)) })
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
