#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

#[macro_use]
pub mod device;

pub mod arch;
pub mod context;
pub mod event;
pub mod exceptions;
pub mod heap;
pub mod memory;
pub mod runtime;
pub mod scheduler;
pub mod sync;
pub mod task;
pub mod thread;
pub mod util;

use device::gic::gic_init_other_cores;
use device::uart;
use sync::SpinLock;

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static INIT_BARRIER: AtomicUsize = AtomicUsize::new(0);
static START_BARRIER: AtomicUsize = AtomicUsize::new(0);
static START_WAIT: AtomicBool = AtomicBool::new(false);

const FLAG_MULTICORE: bool = true;
const FLAG_PREEMPTION: bool = true;

extern "Rust" {
    fn kernel_main(device_tree: device_tree::DeviceTree);
}

#[no_mangle]
pub unsafe extern "C" fn kernel_entry_rust(x0: u32, _x1: u64, _x2: u64, _x3: u64, x4: u32) -> ! {
    unsafe { memory::init() };
    let id = arch::core_id() & 3;

    // TODO: proper heap allocator, and physical memory allocation for heap space
    let heap_base = 0xFFFF_FFFF_FE20_0000 as *mut ();
    unsafe { heap::ALLOCATOR.init(heap_base, 0x20_0000 * 13) };

    let device_tree_base = unsafe { memory::map_physical(x0 as usize, u32::from_be(x4) as usize) };
    let device_tree_base = device_tree_base.as_ptr().cast_const().cast();
    let device_tree = unsafe { device_tree::DeviceTree::load(device_tree_base) }
        .expect("Error parsing device tree");

    device::init_devices(&device_tree);

    unsafe { context::CORES.init() };
    println!("| initialized per-core data");

    println!("| starting other cores...");
    let core_count = device::enable_cpus(&device_tree, arch::boot::kernel_entry_alt);

    INIT_BARRIER.fetch_add(1, Ordering::SeqCst);
    while INIT_BARRIER.load(Ordering::SeqCst) < core_count {
        unsafe { arch::yield_() };
    }

    if FLAG_PREEMPTION {
        println!("| enabling preemption on all cores");
        let preemption_time_ns = 500_000;
        let mut irq = device::IRQ_CONTROLLER.get().lock();
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
        unsafe { arch::yield_() };
    }
    START_WAIT.store(true, Ordering::SeqCst);

    println!("| running event loop on core {id}");
    unsafe { event::run_event_loop() }
}

#[no_mangle]
pub unsafe extern "C" fn kernel_entry_rust_alt(_x0: u32, _x1: u64, _x2: u64, _x3: u64) -> ! {
    let id = arch::core_id() & 3;
    let sp = arch::debug_get_sp();
    println!("| starting core {id}, initial sp {:#x}", sp);

    gic_init_other_cores();

    INIT_BARRIER.fetch_add(1, Ordering::SeqCst);
    START_BARRIER.fetch_add(1, Ordering::SeqCst);
    while !START_WAIT.load(Ordering::SeqCst) {
        unsafe { arch::yield_() };
    }

    if !FLAG_MULTICORE {
        arch::halt();
    }

    println!("| running event loop on core {id}");
    unsafe { event::run_event_loop() }
}

pub fn shutdown() -> ! {
    println!("| shutting down");

    let mut watchdog = device::WATCHDOG.get().lock();
    unsafe { watchdog.reset(63) };
    drop(watchdog);

    arch::halt();
}
