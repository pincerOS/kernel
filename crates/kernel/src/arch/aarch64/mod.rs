use core::arch::{asm, global_asm};

pub mod boot;
pub mod interrupts;
pub mod memory;

pub fn get_time_ticks() -> usize {
    let time;
    unsafe { asm!("mrs {time}, cntpct_el0", time = out(reg) time) };
    time
}
pub fn get_freq_ticks() -> usize {
    let freq;
    unsafe { asm!("mrs {freq}, cntfrq_el0", freq = out(reg) freq) };
    freq
}

extern "C" {
    fn _debug_get_sp() -> usize;
}
global_asm!("_debug_get_sp: mov x0, sp; ret");

pub fn debug_get_sp() -> usize {
    unsafe { _debug_get_sp() }
}

pub fn halt() -> ! {
    unsafe { asm!("1: wfe; b 1b", options(noreturn)) }
}

pub fn core_id() -> u32 {
    let id: u64;
    unsafe { asm!("mrs {id}, mpidr_el1", id = out(reg) id) };
    id as u32
}

pub unsafe fn sev() {
    unsafe { asm!("sev") };
}
pub unsafe fn wfe() {
    // armv8 a-profile reference: G1.19.1 Wait For Event and Send Event
    unsafe { asm!("wfe") };
}
pub unsafe fn yield_() {
    unsafe { asm!("yield") };
}
pub unsafe fn udf() -> ! {
    unsafe { asm!("udf #0", options(noreturn)) };
}
