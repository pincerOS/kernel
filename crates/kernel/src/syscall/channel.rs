use alloc::boxed::Box;
use alloc::sync::Arc;

use crate::event::async_handler::{run_async_handler, HandlerContext};
use crate::event::context::Context;
use crate::process::fd;
use crate::ringbuffer;
use crate::sync::SpinLock;

// TODO: tracking ownership of objects

// TODO: MPMC channels
pub struct Channel {
    pub send: SpinLock<ringbuffer::Sender<16, Message>>,
    pub recv: SpinLock<ringbuffer::Receiver<16, Message>>,
}

pub struct Message {
    pub tag: u64,
    pub objects: [Option<fd::ArcFd>; 4],
    pub data: Option<Box<[u8]>>,
}

#[repr(C)]
pub struct UserMessage {
    pub tag: u64,
    pub objects: [u32; 4],
}

bitflags::bitflags! {
    pub struct SendRecvFlags: u32 {
        const NO_BLOCK = 1 << 0;
    }
}

pub unsafe fn sys_channel(ctx: &mut Context) -> *mut Context {
    let (a_tx, b_rx) = ringbuffer::channel();
    let (b_tx, a_rx) = ringbuffer::channel();
    let a_chan = Channel {
        send: SpinLock::new(a_tx),
        recv: SpinLock::new(a_rx),
    };
    let b_chan = Channel {
        send: SpinLock::new(b_tx),
        recv: SpinLock::new(b_rx),
    };

    let proc = super::current_process().unwrap();

    let mut guard = proc.file_descriptors.lock();
    let a_fdi = guard.insert(Arc::new(a_chan));
    let b_fdi = guard.insert(Arc::new(b_chan));

    ctx.regs[0] = a_fdi;
    ctx.regs[1] = b_fdi;
    ctx
}

pub unsafe fn sys_send(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];
    let msg_ptr = ctx.regs[1];
    let buf_ptr = ctx.regs[2];
    let buf_len = ctx.regs[3];
    let flags = SendRecvFlags::from_bits_truncate(ctx.regs[4] as u32);

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let mut fds_guard = proc.file_descriptors.lock();
        let file = fds_guard.get(fd).cloned();
        let Some(file) = file else {
            drop(fds_guard);
            context.regs().regs[0] = i64::from(-1) as usize;
            return context.resume_final();
        };
        let Some(sender) = file.as_any().downcast_ref::<Channel>() else {
            drop(fds_guard);
            context.regs().regs[0] = i64::from(-1) as usize;
            return context.resume_final();
        };

        let user_message = context.with_user_vmem(|| {
            let msg_ptr = msg_ptr as *const UserMessage;
            assert!(msg_ptr.is_aligned()); // TODO: check user access validity
            unsafe { core::ptr::read(msg_ptr) }
        });

        let mut objects = [const { None }; 4];
        for (&desc, obj) in user_message.objects.iter().zip(&mut objects) {
            if desc != u32::MAX {
                let Some(fd) = fds_guard.remove(desc as usize) else {
                    drop(fds_guard);
                    context.regs().regs[0] = i64::from(-1) as usize;
                    return context.resume_final();
                };
                *obj = Some(fd);
            }
        }

        drop(fds_guard);

        let data;
        if buf_ptr == 0 {
            data = None;
        } else {
            let mut kbuf = Box::new_uninit_slice(buf_len);
            context.with_user_vmem(|| {
                // TODO: validate memory region
                let buf_ptr = buf_ptr as *const u8;
                let kbuf_ptr = kbuf.as_mut_ptr() as *mut u8;
                unsafe {
                    core::ptr::copy_nonoverlapping(buf_ptr, kbuf_ptr, buf_len);
                }
            });
            data = Some(unsafe { kbuf.assume_init() });
        }

        let res;
        if flags.contains(SendRecvFlags::NO_BLOCK) {
            let msg = Message {
                tag: user_message.tag,
                objects,
                data,
            };
            let r = sender.send.lock().try_send(msg);
            if r.is_err() {
                res = -2isize as usize;
            } else {
                res = 0;
            }
        } else {
            let msg = Message {
                tag: user_message.tag,
                objects,
                data,
            };
            sender.send.lock().send(msg).await;
            res = 0;
        }

        context.regs().regs[0] = res;
        context.resume_final()
    })
}

pub unsafe fn sys_recv(ctx: &mut Context) -> *mut Context {
    let fd = ctx.regs[0];
    let msg_ptr = ctx.regs[1];
    let buf_ptr = ctx.regs[2];
    let buf_cap = ctx.regs[3];
    let flags = SendRecvFlags::from_bits_truncate(ctx.regs[4] as u32);

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        let file = proc.file_descriptors.lock().get(fd).cloned();
        let Some(file) = file else {
            context.regs().regs[0] = i64::from(-1) as usize;
            return context.resume_final();
        };
        let Some(channel) = file.as_any().downcast_ref::<Channel>() else {
            context.regs().regs[0] = i64::from(-1) as usize;
            return context.resume_final();
        };

        let message;
        if flags.contains(SendRecvFlags::NO_BLOCK) {
            message = channel.recv.lock().try_recv();
        } else {
            message = Some(channel.recv.lock().recv().await);
        }

        let Some(message) = message else {
            context.regs().regs[0] = -2isize as usize;
            return context.resume_final();
        };

        let mut objects = [u32::MAX; 4];
        {
            let proc = context.cur_process().unwrap();
            let mut fds_guard = proc.file_descriptors.lock();
            for (object, fd) in message.objects.into_iter().zip(&mut objects) {
                if let Some(obj) = object {
                    let new_fd = fds_guard.insert(obj) as u32;
                    *fd = new_fd;
                }
            }
        }

        let user_message = UserMessage {
            tag: message.tag,
            objects,
        };

        let mut data_len = 0;

        context.with_user_vmem(|| {
            if let Some(data) = message.data {
                // TODO: validate memory region
                let buf_ptr = buf_ptr as *mut u8;
                let kbuf_ptr = data.as_ptr();
                let actual_len = data.len().min(buf_cap); // TODO: ??? truncate ???
                unsafe {
                    core::ptr::copy_nonoverlapping(kbuf_ptr, buf_ptr, actual_len);
                }
                data_len = data.len();
            }

            let msg_ptr = msg_ptr as *mut UserMessage;
            assert!(msg_ptr.is_aligned()); // TODO: check user access validity
            unsafe { core::ptr::write(msg_ptr, user_message) };
        });

        context.regs().regs[0] = data_len;
        context.resume_final()
    })
}

impl fd::FileDescriptor for Channel {
    fn is_same_file(&self, other: &dyn fd::FileDescriptor) -> bool {
        let other = other.as_any().downcast_ref::<Self>();
        other.map(|o| core::ptr::eq(self, o)).unwrap_or(true)
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
