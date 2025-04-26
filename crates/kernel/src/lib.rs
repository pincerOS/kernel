#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(unsafe_attr_outside_unsafe, missing_unsafe_on_extern, static_mut_refs)]
#![allow(clippy::new_without_default)]

extern crate alloc;

#[macro_use]
pub mod device;

#[macro_use]
pub mod test;

pub mod arch;
pub mod event;
pub mod fs;
pub mod heap;
pub mod memory;
pub mod process;
pub mod ringbuffer;
pub mod runtime;
pub mod sync;
pub mod syscall;
pub mod util;

use arch::memory::palloc::PAGE_ALLOCATOR;
use device::uart;
use sync::SpinLock;

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static INIT_BARRIER: AtomicUsize = AtomicUsize::new(0);
static START_BARRIER: AtomicUsize = AtomicUsize::new(0);
static START_WAIT: AtomicBool = AtomicBool::new(false);
static INIT_WAIT: AtomicBool = AtomicBool::new(false);

const FLAG_MULTICORE: bool = true;

unsafe extern "Rust" {
    fn kernel_main(device_tree: device_tree::DeviceTree);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_entry_rust(x0: u32, _x1: u64, _x2: u64, _x3: u64, x4: u32) -> ! {
    unsafe { memory::init() };
    let id = arch::core_id() & 3;
    unsafe { sync::enable_interrupts() };

    // TODO: proper heap allocator, and physical memory allocation for heap space
    let heap_base = &raw mut arch::memory::vmm::__rpi_virt_binary_end_addr;
    let heap_end = (&raw mut arch::memory::vmm::__rpi_virt_base).wrapping_byte_add(0x20_0000 * 14);
    let heap_size = unsafe { heap_end.byte_offset_from(heap_base) };

    let bump = unsafe { heap::BumpAllocator::new_uninit() };
    unsafe { bump.init(heap_base.cast(), heap_size as usize) };
    *heap::ALLOCATOR_HACK.lock() = heap::AllocatorHack::Bump(bump);

    unsafe { crate::arch::memory::init_physical_alloc() };

    let device_tree_base = x0 as usize;
    let device_tree_size = u32::from_be(x4) as usize;
    PAGE_ALLOCATOR
        .get()
        .mark_region_unusable(device_tree_base, device_tree_size);
    PAGE_ALLOCATOR
        .get()
        .mark_region_unusable(0x2FF0000, 0x1000 * 8);

    unsafe { crate::arch::memory::vmm::init_kernel_48bit() };
    unsafe { crate::arch::memory::vmm::switch_to_kernel_48bit() };

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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_entry_rust_alt(_x0: u32, _x1: u64, _x2: u64, _x3: u64) -> ! {
    unsafe { sync::enable_interrupts() };

    unsafe { crate::arch::memory::vmm::switch_to_kernel_48bit() };

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
