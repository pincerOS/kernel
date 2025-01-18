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

use device::uart;
use sync::SpinLock;

use core::sync::atomic::{AtomicUsize, Ordering};

static INIT_BARRIER: AtomicUsize = AtomicUsize::new(0);
static START_BARRIER: AtomicUsize = AtomicUsize::new(0);

const FLAG_MULTICORE: bool = true;
const FLAG_PREEMPTION: bool = true;

extern "Rust" {
    fn kernel_main(device_tree: device_tree::DeviceTree);
}

pub fn enable_other_cpus(tree: &device_tree::DeviceTree<'_>, start_fn: unsafe extern "C" fn()) {
    use device_tree::format::StructEntry;
    // TODO: proper discovery through the /cpus/cpu@* path, rather than compatible search
    for mut iter in device::discover_compatible(tree, b"arm,cortex-a53").unwrap() {
        let mut name = None;
        let mut method = None;
        let mut release_addr = None;
        while let Some(Ok(entry)) = iter.next() {
            match entry {
                StructEntry::BeginNode { name: n } => {
                    if name.is_none() {
                        name = Some(n)
                    }
                }
                StructEntry::Prop { name, data } => match name {
                    "enable-method" => method = Some(data),
                    "cpu-release-addr" => {
                        let addr: endian::u64_be = bytemuck::pod_read_unaligned(data);
                        release_addr = Some(addr.get() as usize);
                    }
                    _ => (),
                },
                _ => (),
            }
        }
        match (method, release_addr) {
            (Some(b"spin-table\0"), Some(release_addr)) => {
                println!("| Waking cpu {:?}", name.unwrap_or("unknown"));

                let start = unsafe { memory::map_physical(release_addr, 8).cast::<u64>() };
                let physical_start = memory::physical_addr(start_fn as usize).unwrap();
                unsafe { core::ptr::write_volatile(start.as_ptr(), physical_start) };
                unsafe { arch::sev() };
            }
            _ => println!("| Could not wake cpu {:?}", name),
        }
    }
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
    enable_other_cpus(&device_tree, arch::boot::kernel_entry_alt);

    INIT_BARRIER.fetch_add(1, Ordering::SeqCst);
    while INIT_BARRIER.load(Ordering::SeqCst) < 4 {
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

    println!("| running event loop on core {id}");
    unsafe { event::run_event_loop() }
}

#[no_mangle]
pub unsafe extern "C" fn kernel_entry_rust_alt(_x0: u32, _x1: u64, _x2: u64, _x3: u64) -> ! {
    let id = arch::core_id() & 3;
    let sp = arch::debug_get_sp();
    println!("| starting core {id}, initial sp {:#x}", sp);

    INIT_BARRIER.fetch_add(1, Ordering::SeqCst);
    START_BARRIER.fetch_add(1, Ordering::SeqCst);
    while START_BARRIER.load(Ordering::SeqCst) < 4 {
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
