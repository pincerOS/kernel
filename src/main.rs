#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

use core::arch::{asm, global_asm};

#[macro_use]
mod device;

mod boot;
mod exceptions;
mod heap;
mod runtime;
mod sync;

use device::uart;
use sync::SpinLock;

fn halt() -> ! {
    unsafe { asm!("1: wfe; b 1b", options(noreturn)) }
}

fn core_id() -> u32 {
    let id: u64;
    unsafe { asm!("mrs {id}, mpidr_el1", id = out(reg) id) };
    id as u32
}

static START_BARRIER: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

#[no_mangle]
pub unsafe extern "C" fn kernel_entry_rust(_x0: u32, _x1: u64, _x2: u64, _x3: u64) -> ! {
    let _id = core_id() & 3;

    // TODO: find a proper free region to use as the heap
    // TODO: proper heap allocator, proper heap end bounds
    unsafe { heap::ALLOCATOR.init(0x1_000_000 as *mut ()) };

    // TODO: device tree /soc/serial with compatible:arm,pl011
    let uart_base = 0x3f201000 as *mut ();
    unsafe { uart::UART.init(SpinLock::new(uart::UARTInner::new(uart_base))) };
    println!("| initialized UART");

    // TODO: parse device tree and initialize kernel devices
    // (use the device tree to discover the proper driver and base
    // address of UART, watchdog)

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

    START_BARRIER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    while START_BARRIER.load(core::sync::atomic::Ordering::SeqCst) < 4 {
        unsafe { asm!("yield") }
    }

    kernel_main();
    shutdown();
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

    halt();
}

fn kernel_main() {
    println!("| starting kernel_main");
}

fn shutdown() -> ! {
    println!("| shutting down");
    halt();
}
