#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

#[macro_use]
pub mod device;

pub mod arch;
pub mod event;
pub mod heap;
pub mod memory;
pub mod ringbuffer;
pub mod runtime;
pub mod sync;
pub mod syscall;
pub mod util;

use device::uart;
use sync::SpinLock;

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static INIT_BARRIER: AtomicUsize = AtomicUsize::new(0);
static START_BARRIER: AtomicUsize = AtomicUsize::new(0);
static START_WAIT: AtomicBool = AtomicBool::new(false);
static INIT_WAIT: AtomicBool = AtomicBool::new(false);

const FLAG_MULTICORE: bool = true;

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

    unsafe { event::context::CORES.init() };
    println!("| initialized per-core data");

    println!("| starting other cores...");
    let core_count = device::enable_cpus(&device_tree, arch::boot::kernel_entry_alt);

    INIT_BARRIER.fetch_add(1, Ordering::SeqCst);
    while INIT_BARRIER.load(Ordering::SeqCst) < core_count {
        core::hint::spin_loop();
    }
    INIT_WAIT.store(true, Ordering::SeqCst);

    device::init_devices_per_core();

    println!("| creating initial thread");
    event::thread::thread(move || {
        unsafe { kernel_main(device_tree) };
        shutdown();
    });

    START_BARRIER.fetch_add(1, Ordering::SeqCst);
    while START_BARRIER.load(Ordering::SeqCst) < core_count {
        core::hint::spin_loop();
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

    INIT_BARRIER.fetch_add(1, Ordering::SeqCst);
    while !INIT_WAIT.load(Ordering::SeqCst) {
        core::hint::spin_loop();
    }

    device::init_devices_per_core();

    START_BARRIER.fetch_add(1, Ordering::SeqCst);
    while !START_WAIT.load(Ordering::SeqCst) {
        core::hint::spin_loop();
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
