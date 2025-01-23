#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

#[macro_use]
pub mod device;

pub mod boot;
pub mod exceptions;
pub mod heap;
pub mod memory;
pub mod runtime;
pub mod sync;
pub mod thread;

use device::{timer, uart, watchdog};
use sync::{InterruptSpinLock, SpinLock, UnsafeInit};

use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicUsize, Ordering};

static WATCHDOG: UnsafeInit<SpinLock<watchdog::bcm2835_wdt_driver>> =
    unsafe { UnsafeInit::uninit() };
static IRQ_CONTROLLER: UnsafeInit<InterruptSpinLock<timer::bcm2836_l1_intc_driver>> =
    unsafe { UnsafeInit::uninit() };

static INIT_BARRIER: AtomicUsize = AtomicUsize::new(0);
static START_BARRIER: AtomicUsize = AtomicUsize::new(0);

const FLAG_MULTICORE: bool = true;
const FLAG_PREEMPTION: bool = true;

extern "Rust" {
    fn kernel_main(device_tree: device_tree::DeviceTree);
}

#[no_mangle]
pub unsafe extern "C" fn kernel_entry_rust(x0: u32, _x1: u64, _x2: u64, _x3: u64, x4: u32) -> ! {
    unsafe {
        memory::init();
    }
    let id = core_id() & 3;

    // TODO: proper heap allocator, and physical memory allocation for heap space
    unsafe { heap::ALLOCATOR.init(0xFFFF_FFFF_FE20_0000 as *mut (), 0x20_0000 * 13) };

    // TODO: device tree /soc/serial with compatible:arm,pl011
    let uart_base = unsafe { memory::map_device(0x3f201000) }.as_ptr();
    unsafe { uart::UART.init(SpinLock::new(uart::UARTInner::new(uart_base))) };
    println!("| initialized UART");

    // TODO: device tree /soc/?, compatible:brcm,bcm2835-pm-wdt
    let watchdog_base = unsafe { memory::map_device(0x3f100000) }.as_ptr();
    unsafe {
        let watchdog = watchdog::bcm2835_wdt_driver::init(watchdog_base);
        WATCHDOG.init(SpinLock::new(watchdog));
    }
    println!("| initialized power managment watchdog");
    println!("| last reset: {:#08x}", WATCHDOG.get().lock().last_reset());

    let device_tree_base = unsafe { memory::map_physical(x0 as usize, u32::from_be(x4) as usize) };
    let device_tree =
        unsafe { device_tree::load_device_tree(device_tree_base.as_ptr().cast_const().cast()) }
            .expect("Error parsing device tree");

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
    let other_core_start = unsafe { memory::map_physical(0xd8, 4 * 8) }
        .as_ptr()
        .cast::<u64>();
    let physical_alt_start =
        memory::physical_addr(boot::kernel_entry_alt as usize).expect("boot code should be mapped");
    for i in 1..4 {
        let target = other_core_start.wrapping_add(i);
        unsafe { core::ptr::write_volatile(target, physical_alt_start) };
    }
    unsafe {
        asm!("sev");
    }

    println!("| initializing interrupt controller");
    let timer_base = unsafe { memory::map_device(0x40000000) }.as_ptr();
    let timer = device::timer::bcm2836_l1_intc_driver::new(timer_base);
    let timer = sync::InterruptSpinLock::new(timer);
    unsafe { IRQ_CONTROLLER.init(timer) };

    INIT_BARRIER.fetch_add(1, Ordering::SeqCst);
    while INIT_BARRIER.load(Ordering::SeqCst) < 4 {
        unsafe { asm!("yield") }
    }

    if FLAG_PREEMPTION {
        println!("| enabling preemption on all cores");
        let preemption_time_ns = 500_000;
        let mut irq = IRQ_CONTROLLER.get().lock();
        for core in 0..4 {
            irq.start_timer(core, preemption_time_ns);
        }
    }

    println!("| creating initial thread");
    thread::thread(move || {
        unsafe { kernel_main(device_tree) };
        shutdown();
    });

    START_BARRIER.fetch_add(1, Ordering::SeqCst);
    while START_BARRIER.load(Ordering::SeqCst) < 4 {
        unsafe { asm!("yield") }
    }

    println!("| running threads on core {id}");
    unsafe { thread::SCHEDULER.run_on_core() }
}

#[no_mangle]
pub unsafe extern "C" fn kernel_entry_rust_alt(_x0: u32, _x1: u64, _x2: u64, _x3: u64) -> ! {
    let id = core_id() & 3;
    let sp = unsafe { get_sp() };
    println!("| starting core {id}, initial sp {:#x}", sp);

    INIT_BARRIER.fetch_add(1, Ordering::SeqCst);
    START_BARRIER.fetch_add(1, Ordering::SeqCst);
    while START_BARRIER.load(Ordering::SeqCst) < 4 {
        unsafe { asm!("yield") }
    }

    if !FLAG_MULTICORE {
        halt();
    }

    println!("| running threads on core {id}");
    unsafe { thread::SCHEDULER.run_on_core() }
}

fn shutdown() -> ! {
    println!("| shutting down");

    let mut watchdog = WATCHDOG.get().lock();
    unsafe {
        watchdog.reset(63);
    }

    halt();
}

extern "C" {
    fn get_sp() -> usize;
}
global_asm!("get_sp: mov x0, sp; ret");

fn halt() -> ! {
    unsafe { asm!("1: wfe; b 1b", options(noreturn)) }
}

fn core_id() -> u32 {
    let id: u64;
    unsafe { asm!("mrs {id}, mpidr_el1", id = out(reg) id) };
    id as u32
}
