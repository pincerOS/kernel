use core::num::NonZeroU32;

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::event::async_handler::{run_async_handler, HandlerContext};
use crate::event::context::Context;
use crate::ringbuffer;
use crate::sync::SpinLock;

// TODO: tracking ownership of objects
pub struct ObjectDescriptor(core::num::NonZeroU32);

pub struct Message {
    pub tag: u64,
    pub objects: [Option<ObjectDescriptor>; 4],
    pub data: Option<Box<[u8]>>,
}

pub enum Object {
    Channel {
        send: ringbuffer::Sender<16, Message>,
        recv: ringbuffer::Receiver<16, Message>,
    },
}

// TODO: this is a hack, move it to a per-task list & remap in messages
pub static OBJECTS: SpinLock<Vec<Option<Object>>> = SpinLock::new(Vec::new());

pub fn alloc_obj(obj: Object) -> ObjectDescriptor {
    let mut list = OBJECTS.lock();
    let idx = list.len();
    list.push(Some(obj));
    ObjectDescriptor(core::num::NonZeroU32::new(idx as u32).unwrap())
}

pub unsafe fn sys_channel(ctx: &mut Context) -> *mut Context {
    let (a_tx, b_rx) = ringbuffer::channel();
    let (b_tx, a_rx) = ringbuffer::channel();
    let a_chan = alloc_obj(Object::Channel {
        send: a_tx,
        recv: a_rx,
    });
    let b_chan = alloc_obj(Object::Channel {
        send: b_tx,
        recv: b_rx,
    });

    ctx.regs[0] = a_chan.0.get() as usize;
    ctx.regs[1] = b_chan.0.get() as usize;

    ctx
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

pub unsafe fn sys_send(ctx: &mut Context) -> *mut Context {
    let desc = ctx.regs[0];
    let msg_ptr = ctx.regs[1];
    let buf_ptr = ctx.regs[2];
    let buf_len = ctx.regs[3];
    let flags = SendRecvFlags::from_bits_truncate(ctx.regs[4] as u32);

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let Some(desc) = NonZeroU32::new(desc as u32) else {
            context.regs().regs[0] = -1isize as usize;
            return context.resume_final();
        };
        let sender = OBJECTS
            .lock()
            .get_mut(desc.get() as usize)
            .and_then(|d| match d.take() {
                Some(Object::Channel { send, recv }) => Some((send, recv)),
                obj => {
                    *d = obj;
                    None
                }
            });
        let Some((mut send, recv)) = sender else {
            // TODO: proper approach to mpsc channels?
            println!("Skipping message, channel in-use or non-existant!");
            context.regs().regs[0] = -1isize as usize;
            return context.resume_final();
        };

        let user_message = context.with_user_vmem(|| {
            let msg_ptr = msg_ptr as *const UserMessage;
            assert!(msg_ptr.is_aligned()); // TODO: check user access validity
            unsafe { core::ptr::read(msg_ptr) }
        });

        // TODO: validate ownership of objects
        let objects = user_message
            .objects
            .map(NonZeroU32::new)
            .map(|d| d.map(ObjectDescriptor));

        let data;
        if buf_ptr == 0 {
            data = None;
        } else {
            // TODO: validate memory region
            let buf_ptr = buf_ptr as *const u8;
            let mut kbuf = Box::new_uninit_slice(buf_len);
            let kbuf_ptr = kbuf.as_mut_ptr() as *mut u8;
            unsafe {
                core::ptr::copy_nonoverlapping(buf_ptr, kbuf_ptr, buf_len);
            }
            data = Some(unsafe { kbuf.assume_init() });
        }

        let res;
        if flags.contains(SendRecvFlags::NO_BLOCK) {
            let r = send.try_send(Message {
                tag: user_message.tag,
                objects,
                data,
            });
            if r.is_err() {
                res = -2isize as usize;
            } else {
                res = 0;
            }
        } else {
            send.send(Message {
                tag: user_message.tag,
                objects,
                data,
            })
            .await;
            res = 0;
        }

        OBJECTS.lock()[desc.get() as usize] = Some(Object::Channel { send, recv });

        context.regs().regs[0] = res;
        context.resume_final()
    })
}

pub unsafe fn sys_recv(ctx: &mut Context) -> *mut Context {
    let desc = ctx.regs[0];
    let msg_ptr = ctx.regs[1];
    let buf_ptr = ctx.regs[2];
    let buf_cap = ctx.regs[3];
    let flags = SendRecvFlags::from_bits_truncate(ctx.regs[4] as u32);

    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let Some(desc) = NonZeroU32::new(desc as u32) else {
            context.regs().regs[0] = -1isize as usize;
            return context.resume_final();
        };
        let receiver = OBJECTS
            .lock()
            .get_mut(desc.get() as usize)
            .and_then(|d| match d.take() {
                Some(Object::Channel { send, recv }) => Some((send, recv)),
                obj => {
                    *d = obj;
                    None
                }
            });

        let Some((send, mut recv)) = receiver else {
            // TODO: proper approach to mpsc channels?
            println!("Skipping message, channel in-use or non-existant!");
            context.regs().regs[0] = -1isize as usize;
            return context.resume_final();
        };

        let message;
        if flags.contains(SendRecvFlags::NO_BLOCK) {
            message = recv.try_recv();
        } else {
            message = Some(recv.recv().await);
        }

        OBJECTS.lock()[desc.get() as usize] = Some(Object::Channel { send, recv });

        let Some(message) = message else {
            context.regs().regs[0] = -2isize as usize;
            return context.resume_final();
        };

        let user_message = UserMessage {
            tag: message.tag,
            objects: message.objects.map(|s| s.map(|o| o.0.get()).unwrap_or(0)),
        };
        // TODO: track ownership of objects

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
