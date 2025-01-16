#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

use core::arch::{asm, global_asm};

#[macro_use]
mod device;

mod boot;
mod dtb;
mod exceptions;
mod heap;
mod runtime;
mod sync;
mod thread;

use device::{mailbox, timer, uart, watchdog};
use sync::{InterruptSpinLock, SpinLock, UnsafeInit};

fn halt() -> ! {
    unsafe { asm!("1: wfe; b 1b", options(noreturn)) }
}

fn core_id() -> u32 {
    let id: u64;
    unsafe { asm!("mrs {id}, mpidr_el1", id = out(reg) id) };
    id as u32
}

static WATCHDOG: UnsafeInit<SpinLock<watchdog::bcm2835_wdt_driver>> =
    unsafe { UnsafeInit::uninit() };
static IRQ_CONTROLLER: UnsafeInit<InterruptSpinLock<timer::bcm2836_l1_intc_driver>> =
    unsafe { UnsafeInit::uninit() };

static START_BARRIER: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

#[no_mangle]
pub unsafe extern "C" fn kernel_entry_rust(x0: u32, _x1: u64, _x2: u64, _x3: u64) -> ! {
    let id = core_id() & 3;

    // TODO: find a proper free region to use as the heap
    // TODO: proper heap allocator, proper heap end bounds
    unsafe { heap::ALLOCATOR.init(0x1_000_000 as *mut ()) };

    // TODO: device tree /soc/serial with compatible:arm,pl011
    let uart_base = 0x3f201000 as *mut ();
    unsafe { uart::UART.init(SpinLock::new(uart::UARTInner::new(uart_base))) };
    println!("| initialized UART");

    // TODO: device tree /soc/?, compatible:brcm,bcm2835-pm-wdt
    let watchdog_base = 0x3f100000 as *mut ();
    unsafe {
        let watchdog = watchdog::bcm2835_wdt_driver::init(watchdog_base);
        WATCHDOG.init(SpinLock::new(watchdog));
    }
    println!("| initialized power managment watchdog");
    println!("| last reset: {:#08x}", WATCHDOG.get().lock().last_reset());

    let device_tree =
        unsafe { dtb::load_device_tree(x0 as *const u64) }.expect("Error parsing device tree");

    // TODO: parse device tree and initialize kernel devices
    // (use the device tree to discover the proper driver and base
    // address of UART, watchdog)

    thread::CORES.init();

    println!("| initialized per-core data");

    println!("| starting other cores...");
    // Start other cores; the bootloader has them waiting in a WFE loop,
    // checking 0xd8 + core_id
    // TODO: use device tree to discover 1. enable-method for the cpu
    // and 2. the cpu-release-addr (the address it's spinning at)
    let other_core_start = 0xd8 as *mut unsafe extern "C" fn();
    for i in 1..4 {
        let target = other_core_start.wrapping_add(i);
        unsafe { core::ptr::write_volatile(target, boot::kernel_entry_alt) };
    }
    unsafe {
        asm!("sev");
    }

    println!("| creating initial thread");

    thread::thread(move || {
        kernel_main(device_tree);
        shutdown();
    });

    START_BARRIER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    while START_BARRIER.load(core::sync::atomic::Ordering::SeqCst) < 4 {
        unsafe { asm!("yield") }
    }

    println!("| running threads on core {id}");
    unsafe { thread::SCHEDULER.run_on_core() }
}

extern "C" {
    fn get_sp() -> usize;
}
global_asm!("get_sp: mov x0, sp; ret");

#[no_mangle]
pub unsafe extern "C" fn kernel_entry_rust_alt(_x0: u32, _x1: u64, _x2: u64, _x3: u64) -> ! {
    let id = core_id() & 3;
    let sp = unsafe { get_sp() };
    println!("| starting core {id}, initial sp {:#x}", sp);

    START_BARRIER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    while START_BARRIER.load(core::sync::atomic::Ordering::SeqCst) < 4 {
        unsafe { asm!("yield") }
    }

    println!("| running threads on core {id}");
    unsafe { thread::SCHEDULER.run_on_core() }
}

extern "C" {
    fn example_syscall(a: usize, b: usize) -> usize;
}
global_asm!("example_syscall: svc #1; ret");

fn kernel_main(device_tree: dtb::DeviceTree) {
    println!("| starting kernel_main");

    let (a, b) = (5, 7);
    let c = unsafe { example_syscall(a, b) };
    println!("syscall #1 ({a}, {b}) -> {c}");

    println!("Reserved regions:");
    for region in device_tree.reserved_regions {
        println!(
            "  {:#010} (size {:x})",
            region.address.get(),
            region.size.get()
        );
    }

    // dtb::debug_device_tree(device_tree).unwrap();

    // TODO: find mailbox address via device tree
    let mailbox_base = 0x3f00b880 as *mut ();
    let mut mailbox = unsafe { mailbox::VideoCoreMailbox::init(mailbox_base) };

    let timer = device::timer::bcm2836_l1_intc_driver::new(0x40000000 as *mut ());
    let timer = sync::InterruptSpinLock::new(timer);
    unsafe {
        IRQ_CONTROLLER.init(timer);
    }

    let preemption_time_ns = 500_000;
    {
        let mut irq = IRQ_CONTROLLER.get().lock();
        for core in 0..4 {
            irq.start_timer(core, preemption_time_ns);
        }
    }

    // Basic preemption test
    let count = 32;
    let barrier = alloc::sync::Arc::new(sync::Barrier::new(count + 1));

    for i in 0..count {
        let b = barrier.clone();
        thread::thread(move || {
            println!("Starting thread {i}");
            sync::spin_sleep(500_000);
            println!("Ending thread {i}");
            b.sync();
        });
    }
    barrier.sync();
    println!("End of preemption test");

    let mut surface = unsafe { mailbox.get_framebuffer() };

    vsync_tearing_demo(&mut surface);
}

fn vsync_tearing_demo(surface: &mut mailbox::Surface) {
    let (width, height) = surface.dimensions();

    for i in 0.. {
        let color = 0xFFFF0000 | (i as i32 % 512 - 256).abs().min(255) as u32;
        let color2 = 0xFF0000FF | ((i as i32 % 512 - 256).abs().min(255) as u32) << 16;
        let stripe_width = width / 20;
        let offset = i * (120 / surface.framerate());
        for r in 0..height {
            for c in 0..width {
                let cluster = (c + offset % (2 * stripe_width)) / stripe_width;
                let color = if cluster % 2 == 0 { color } else { color2 };
                surface[(r, c)] = color;
            }
        }

        surface.present();
        surface.wait_for_frame();
    }
}

fn shutdown() -> ! {
    println!("| shutting down");

    let mut watchdog = WATCHDOG.get().lock();
    unsafe {
        watchdog.reset(63);
    }

    halt();
}
