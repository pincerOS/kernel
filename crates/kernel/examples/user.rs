#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::collections::btree_map::BTreeMap;
use core::arch::asm;
use core::mem::MaybeUninit;
use core::num::NonZeroU32;
use core::ptr::copy_nonoverlapping;
use core::sync::atomic::{AtomicU32, Ordering};

use kernel::*;

use context::{deschedule_thread, Context, DescheduleAction, CORES};
use sync::SpinLock;

unsafe fn sys_shutdown(_ctx: &mut Context) -> *mut Context {
    shutdown();
}
unsafe fn sys_hello(ctx: &mut Context) -> *mut Context {
    println!("Hello world syscall");
    ctx
}
unsafe fn sys_yield(ctx: &mut Context) -> *mut Context {
    let (core_sp, thread) = CORES.with_current(|core| (core.core_sp.get(), core.thread.take()));
    let mut thread = thread.expect("usermode syscall without active thread");
    unsafe { thread.save_context(ctx.into()) };

    let action = DescheduleAction::Yield;
    unsafe { deschedule_thread(core_sp, thread, action) }
}
unsafe fn sys_print(ctx: &mut Context) -> *mut Context {
    let ptr = ctx.regs[0];
    let len = ctx.regs[1];
    // TODO: soundness (check user permissions for the range)
    let data = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    let mut stdout = device::uart::UART.get().lock();
    for c in data {
        stdout.writec(*c);
    }
    ctx
}
unsafe fn sys_exit(ctx: &mut Context) -> *mut Context {
    let (core_sp, thread) = CORES.with_current(|core| (core.core_sp.get(), core.thread.take()));
    let mut thread = thread.expect("usermode syscall without active thread");
    unsafe { thread.save_context(ctx.into()) };

    let action = DescheduleAction::FreeThread;
    unsafe { deschedule_thread(core_sp, thread, action) }
}
unsafe fn sys_spawn(ctx: &mut Context) -> *mut Context {
    let user_entry = ctx.regs[0];
    let user_sp = ctx.regs[1];
    let flags = ctx.regs[2];

    let cur_page_dir = CORES.with_current(|core| {
        let thread = core.thread.take().unwrap();
        let page_dir = thread.user_regs.as_ref().unwrap().ttbr0_el1;
        core.thread.set(Some(thread));
        page_dir
    });

    let page_dir;
    if flags == 1 {
        // shared mem
        page_dir = cur_page_dir;
    } else {
        // fork-style
        let (dst_data, new_page_dir) = crate::arch::memory::create_user_region();
        let dst_data = dst_data as *mut u8;
        // This is a massive hack
        let buf_size = 1 << 16;
        let mut buffer: Box<[MaybeUninit<u8>]> = Box::new_uninit_slice(buf_size);

        let buf_ptr = buffer.as_mut_ptr().cast();
        let src_data = 0x20_0000 as *const u8;
        let src_size = 0x20_0000 * 15;
        for i in 0..(src_size / buf_size) {
            unsafe {
                copy_nonoverlapping(src_data.byte_add(i * buf_size), buf_ptr, buf_size);
                asm!("msr TTBR0_EL1, {0}", "isb", in(reg) new_page_dir);
                copy_nonoverlapping(buf_ptr, dst_data.byte_add(i * buf_size), buf_size);
                asm!("msr TTBR0_EL1, {0}", "isb", in(reg) cur_page_dir);
            }
        }
        page_dir = cur_page_dir;
    }

    let user_thread = unsafe { thread::Thread::new_user(user_sp, user_entry, page_dir) };
    event::SCHEDULER.add_task(event::Event::ScheduleThread(user_thread));

    ctx
}

static CHANNELS: SpinLock<BTreeMap<u32, Vec<Option<Object>>>> = 
    SpinLock::new(BTreeMap::new());

// TODO: employ hashing and concat with host in order to make them unique
static NEXT_CHANNEL_ID: AtomicU32 = AtomicU32::new(1);
struct ObjectDescriptor {
    id: u32,
    chained: Option<Box<ObjectDescriptor>>,
}

struct Message {
    tag: u64,
    objects: [Option<ObjectDescriptor>; 4],
    data: Option<Box<[u8]>>,
}

enum Object {
    Sender(ringbuffer::Sender<16, Message>),
    Receiver(ringbuffer::Receiver<16, Message>),
}

// TODO: make dealloc_obj/take method instead of doing it twice. generalize it as well
// make method for ownership transfer

fn alloc_obj(task_id: u32, obj: Object) -> ObjectDescriptor {
    let mut map = CHANNELS.lock();
    let list = map.entry(task_id).or_insert_with(Vec::new);

    // get the next channel id
    let chan_id = NEXT_CHANNEL_ID.fetch_add(1, Ordering::Relaxed);
    list.push(Some(obj));

    // ask alex about this lol im stupid ;-;
    return ObjectDescriptor { id: chan_id, chained: None };
                                
}

unsafe fn sys_channel(ctx: &mut Context) -> *mut Context {
    let (_, thread) = CORES.with_current(|core| (core.core_sp.get(), core.thread.take()));

    let task_id = match thread {
        Some(t) => t.id,
        None => {
            ctx.regs[0] = (-1isize) as usize;
            return ctx;
        }
    };

    let (tx, rx) = ringbuffer::channel();
    let tx = alloc_obj(task_id, Object::Sender(tx));
    let rx = alloc_obj(task_id, Object::Receiver(rx));

    tx.chained = Option::new(Box::new(rx));
    rx.chained = Option::new(Box::new(tx));

    ctx.regs[0] = tx.id.get() as usize;
    ctx.regs[1] = rx.id.get() as usize;

    ctx
}

#[repr(C)]
struct UserMessage {
    tag: u64,
    objects: [u32; 4],
}

// bank for holding unused channel descriptors

unsafe fn sys_transact(ctx: &mut Context) -> *mut Context {
    let desc = ctx.regs[0];
    let slip = ctx.regs[1];
    let flags = ctx.regs[2];

    // if flag == deposit
    // return token
    // if flag == withdrawl
    // return fd
}

unsafe fn sys_send(ctx: &mut Context) -> *mut Context {
    let desc = ctx.regs[0];
    let msg_ptr = ctx.regs[1];
    let buf_ptr = ctx.regs[2];
    let buf_len = ctx.regs[3];

    let Some(_desc) = NonZeroU32::new(desc as u32) else {
        ctx.regs[0] = (-1isize) as usize;
        return ctx;
    };

    let (_, thread) = CORES.with_current(|core| (core.core_sp.get(), core.thread.take()));

    let task_id = match thread {
        Some(t) => t.id,
        None => {
            ctx.regs[0] = (-1isize) as usize;
            return ctx;
        }
    };

    let mut objects = CHANNELS.lock();
    let Some(descriptors) = objects.get_mut(&task_id) else {
        ctx.regs[0] = (-1isize) as usize;
        return ctx;
    };

    // Find a sender object in the vector
    let Some(slot) = descriptors.iter_mut().find(|d| matches!(d, Some(Object::Sender(_)))) else {
        ctx.regs[0] = (-1isize) as usize;
        return ctx;
    };

    // Extract the sender from the Option<Object> and make it mutable
    let Object::Sender(mut sender) = slot.take().unwrap() else {
        ctx.regs[0] = (-1isize) as usize;
        return ctx;
    };

    let msg_ptr = msg_ptr as *const UserMessage;
    assert!(msg_ptr.is_aligned());
    let user_message = unsafe { core::ptr::read(msg_ptr) };

    // TODO: validate ownership of objects
    // let objects = user_message
    //     .objects
    //     .map(NonZeroU32::new)
    //     .map(|d| d.map(ObjectDescriptor));

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
        data = Some(kbuf.assume_init());
    }

    let res = sender.try_send(Message {
        tag: user_message.tag,
        objects,
        data,
    });

    if res.is_ok() {
        ctx.regs[0] = 0 as usize;
    } else {
        ctx.regs[0] = (-1isize) as usize;
    }

    ctx
}

unsafe fn sys_recv(ctx: &mut Context) -> *mut Context {
    let desc = ctx.regs[0];
    let msg_ptr = ctx.regs[1];
    let buf_ptr = ctx.regs[2];
    let buf_cap = ctx.regs[3];

    let Some(_desc) = NonZeroU32::new(desc as u32) else {
        ctx.regs[0] = (-1isize) as usize;
        return ctx;
    };

    let (_, thread) = CORES.with_current(|core| (core.core_sp.get(), core.thread.take()));

    let task_id = match thread {
        Some(t) => t.id,
        None => {
            ctx.regs[0] = (-1isize) as usize;
            return ctx;
        }
    };

    let mut objects = CHANNELS.lock();
    let Some(descriptors) = objects.get_mut(&task_id) else {
        ctx.regs[0] = (-1isize) as usize;
        return ctx;
    };

    // Find a receiver object in the vector
    let Some(slot) = descriptors.iter_mut().find(|d| matches!(d, Some(Object::Receiver(_)))) else {
        ctx.regs[0] = (-1isize) as usize;
        return ctx;
    };

    // Extract the receiver from the Option<Object>
    let Object::Receiver(mut receiver) = slot.take().unwrap() else {
        ctx.regs[0] = (-1isize) as usize;
        return ctx;
    };

    // Attempt to receive a message
    let message = match receiver.try_recv() {
        Some(m) => m,
        None => {
            *slot = Some(Object::Receiver(receiver)); // Put it back before returning
            ctx.regs[0] = (-2isize) as usize;
            return ctx;
        }
    };

    let user_message = UserMessage {
        tag: message.tag,
        objects: message.objects.map(|s| s.map(|o| o.0.get()).unwrap_or(0)),
    };

    // TODO: track ownership of objects

    let mut data_len = 0;
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

    ctx.regs[0] = data_len;

    ctx
}

static INIT_CODE: &[u8] = kernel::util::include_bytes_align!(u32, "../../init/init.bin");

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    unsafe {
        exceptions::register_syscall_handler(1, sys_shutdown);
        exceptions::register_syscall_handler(2, sys_hello);
        exceptions::register_syscall_handler(3, sys_yield);
        exceptions::register_syscall_handler(4, sys_print);
        exceptions::register_syscall_handler(5, sys_spawn);
        exceptions::register_syscall_handler(6, sys_exit);
        exceptions::register_syscall_handler(7, sys_channel);
        exceptions::register_syscall_handler(8, sys_send);
        exceptions::register_syscall_handler(9, sys_recv);
    }

    unsafe { crate::arch::memory::init_physical_alloc() };

    // Create user region (mapped at 0x20_0000 in virtual memory)
    let (user_region, ttbr0) = unsafe { crate::arch::memory::create_user_region() };

    // Mark current thread as using TTBR0, so that preemption saves
    // and restores the register.
    context::CORES.with_current(|core| {
        let mut thread = core.thread.take().unwrap();
        thread.user_regs = Some(thread::UserRegs {
            sp_el0: 0,
            ttbr0_el1: ttbr0,
            usermode: false,
        });
        core.thread.set(Some(thread));
    });
    // Enable the user-mode address space in this thread
    unsafe { asm!("msr TTBR0_EL1, {0}", "isb", in(reg) ttbr0) };

    println!("User ptr: {:p}", user_region);
    // TODO: sometimes get an insn abort here? (leads to UART deadlock)
    println!(
        "Physical addr: {:?}",
        memory::physical_addr(user_region.addr())
    );
    let access = crate::arch::memory::at_s1e0r(user_region.addr());
    println!(
        "user access: {:?}",
        access.map(|b| b.bits()).map_err(|e| e.bits())
    );

    let start = sync::get_time();
    {
        let code_src = bytemuck::cast_slice::<_, u32>(INIT_CODE);

        let user_code_ptr = user_region.cast::<u32>();
        let user_code = unsafe { core::slice::from_raw_parts_mut(user_code_ptr, code_src.len()) };
        user_code.copy_from_slice(code_src);
    }
    let end = sync::get_time();

    // TODO: this sometimes takes significantly longer?
    // "Done copying user data, took 868749µs"
    println!("Done copying user data, took {:4}µs", end - start);

    let user_sp = 0x100_0000;
    let user_entry = 0x20_0000;

    let user_thread = unsafe { thread::Thread::new_user(user_sp, user_entry, ttbr0) };

    event::SCHEDULER.add_task(event::Event::ScheduleThread(user_thread));

    thread::stop();
}
