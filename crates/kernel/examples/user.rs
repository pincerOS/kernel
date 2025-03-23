#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use core::arch::asm;

use alloc::sync::Arc;
use event::{context, task, thread};
use kernel::*;

static INIT_CODE: &[u8] = kernel::util::include_bytes_align!(u32, "../../init/init.bin");

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    unsafe { syscall::register_syscalls() };
    unsafe { crate::arch::memory::init_physical_alloc() };

    let (_stdio, mut stdin_tx, mut stdout_rx) = {
        let (stdin_tx, stdin_rx) = ringbuffer::channel();
        let (stdout_tx, stdout_rx) = ringbuffer::channel();
        let stdio_chan = syscall::channel::alloc_obj(syscall::channel::Object::Channel {
            send: stdout_tx,
            recv: stdin_rx,
        });
        (stdio_chan, stdin_tx, stdout_rx)
    };

    task::spawn_async(async move {
        let mut buf = [0; 256];
        let mut buf_len = 0;
        loop {
            {
                let uart = device::uart::UART.get();
                let mut guard = uart.lock();
                while let Some(c) = guard.try_getc() {
                    buf[buf_len] = c;
                    buf_len += 1;
                    if buf_len >= 256 {
                        break;
                    }
                }
            }
            if buf_len > 0 {
                let msg = syscall::channel::Message {
                    tag: 0,
                    objects: [const { None }; 4],
                    data: Some(buf[..buf_len].into()),
                };
                stdin_tx.send(msg).await;
                buf_len = 0;
            }
            task::yield_future().await;
        }
    });
    task::spawn_async(async move {
        loop {
            let input = stdout_rx.recv().await;
            if let Some(data) = input.data {
                let uart = device::uart::UART.get();
                let mut stdout = uart.lock();
                for c in data {
                    stdout.writec(c);
                }
            }
        }
    });

    // Create a user process
    let mut process = crate::process::Process::new();
    // Assume fixed mapped range in user process (0x20_0000 in virtual memory)
    process
        .page_table
        .lock()
        .reserve_memory_range(0x20_000, 0x20_000 * 7, false);
    let user_region = 0x20_0000 as *mut u8;
    let ttbr0 = process.get_ttbr0();

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
    unsafe {
        asm!("msr TTBR0_EL1, {0}", "isb", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) ttbr0)
    };

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
    unsafe {
        core::ptr::copy_nonoverlapping(
            INIT_CODE.as_ptr(),
            user_region.cast::<u8>(),
            INIT_CODE.len(),
        );
    }
    let end = sync::get_time();

    // TODO: this sometimes takes significantly longer?
    // "Done copying user data, took 868749µs"
    println!("Done copying user data, took {:4}µs", end - start);

    static ARCHIVE: &[u8] = initfs::include_bytes_align!(u32, "../../init/fs.arc");
    let fs = fs::initfs::InitFs::new(&ARCHIVE).unwrap();
    let root = fs.root();

    process.root = Some(root.clone());

    {
        let mut fds = process.file_descriptors.lock();
        let uart_fd = Arc::new(process::fd::UartFd(device::uart::UART.get())) as Arc<_>;
        let _ = fds.set(0, Arc::clone(&uart_fd));
        let _ = fds.set(1, Arc::clone(&uart_fd));
        let _ = fds.set(2, uart_fd);
        let _ = fds.set(3, root);
    }

    let user_sp = 0x100_0000;
    let user_entry = 0x20_0000;

    let user_thread = unsafe { thread::Thread::new_user(Arc::new(process), user_sp, user_entry) };

    event::SCHEDULER.add_task(event::Event::ScheduleThread(user_thread));

    thread::stop();
}
