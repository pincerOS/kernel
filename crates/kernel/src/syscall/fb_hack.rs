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

#[repr(C)]
#[derive(Copy, Clone)]
pub struct InputEvent {
    pub time: u64,
    pub kind: u16,
    pub code: u16,
    pub value: u32,
}

unsafe impl bytemuck::Zeroable for InputEvent {}
unsafe impl bytemuck::Pod for InputEvent {}

pub const EVENT_KEY: u16 = 0x01;
pub const EVENT_RELATIVE: u16 = 0x02;

pub const REL_XY: u16 = 0x01;
pub const REL_WHEEL: u16 = 0x02;

pub trait InputSource {
    fn next<'a>(&'a self) -> fd::SmallFuture<'a, Result<InputEvent, ()>>;
}

pub struct MouseInput;

impl InputSource for MouseInput {
    fn next<'a>(&'a self) -> fd::SmallFuture<'a, Result<InputEvent, ()>> {
        use crate::device::usb::mouse::{MouseButton, MouseEvent, MOUSE_EVENTS};

        // TODO: proper async interface
        if let Some(event) = MOUSE_EVENTS.poll() {
            let time = crate::sync::get_time() as u64;
            let event = match event {
                MouseEvent::Move { x, y } => InputEvent {
                    time,
                    kind: EVENT_RELATIVE,
                    code: REL_XY,
                    value: ((y as i16 as u16 as u32) << 16) | (x as i16 as u16 as u32),
                },
                MouseEvent::Button {
                    button,
                    state,
                    all: _,
                } => {
                    let button = match button {
                        MouseButton::Left => 1,
                        MouseButton::Right => 2,
                        MouseButton::Middle => 3,
                        MouseButton::M4 => 4,
                        MouseButton::M5 => 5,
                    };
                    InputEvent {
                        time,
                        kind: EVENT_KEY,
                        code: 0x1000 | button,
                        value: state as u32,
                    }
                }
                MouseEvent::Wheel { delta } => InputEvent {
                    time,
                    kind: EVENT_RELATIVE,
                    code: REL_WHEEL,
                    value: delta as i32 as u32,
                },
            };
            fd::boxed_future(async move { Ok(event) })
        } else {
            fd::boxed_future(async move { Err(()) })
        }
    }
}

pub struct KeyboardInput;

impl InputSource for KeyboardInput {
    fn next<'a>(&'a self) -> fd::SmallFuture<'a, Result<InputEvent, ()>> {
        use crate::device::usb::keyboard::KEY_EVENTS;
        // TODO: proper async interface
        if let Some(event) = KEY_EVENTS.poll() {
            let time = crate::sync::get_time() as u64;
            let event = InputEvent {
                time,
                kind: EVENT_KEY,
                code: event.code as u16,
                value: event.pressed as u32,
            };
            fd::boxed_future(async move { Ok(event) })
        } else {
            fd::boxed_future(async move { Err(()) })
        }
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

pub unsafe fn sys_poll_mouse_event(ctx: &mut Context) -> *mut Context {
    let buf_ptr = ctx.regs[0];
    let buf_len = ctx.regs[1].min(u32::MAX as usize) as u32;

    // TODO: expose this as a /dev vfs file
    run_async_handler(ctx, async move |context: HandlerContext<'_>| {
        let event = MouseInput.next().await;

        if let Ok(ev) = event {
            // TODO: sound abstraction for usermode buffers...
            // (prevent TOCTOU issues, pin pages to prevent user unmapping them,
            // deal with unmapped pages...)
            // TODO: check user buffers
            context.with_user_vmem(|| {
                let buf = unsafe {
                    core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len as usize)
                };
                buf[..size_of::<InputEvent>()].copy_from_slice(bytemuck::bytes_of(&ev));
            });
            context.resume_return(0)
        } else {
            context.resume_return(-1isize as usize)
        }
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

pub struct MemFd {
    pages: SpinLock<BTreeMap<usize, PhysicalPage<Size4KiB>>>,
}

impl MemFd {
    fn new() -> Self {
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
