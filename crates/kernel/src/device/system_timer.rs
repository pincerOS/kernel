use crate::arch;
use crate::context::Context;
use crate::sync::Volatile;
/// A system timer driver for the raspi4b.
/// This is the Bcm2835-system-timer
///
/// The bcm2835-system-timer was implemented as the primary timer instead of the ARM Generic Timer
///     as the BCM2711 Peripherals Manual states that the system timer is more accurate (and the arm generic timer is based off of it)
///     and therefore the system timer should be used for time keeping
///
///
use core::arch::asm;
use core::panic;

use super::gic::gic_register_isr;

use crate::sync::UnsafeInit;
static SYSTEM_TIMER: UnsafeInit<Bcm2835SysTmr> = unsafe { UnsafeInit::uninit() };

//Please help
static mut ARM_GENERIC_TIMERS: [ArmGenericTimer; 4] = [
    ArmGenericTimer::new(),
    ArmGenericTimer::new(),
    ArmGenericTimer::new(),
    ArmGenericTimer::new(),
];

/// Run on each core
pub fn register_arm_generic_timer_irqs() {
    gic_register_isr(30, arm_generic_timer_irq_handler);

    //Initialize the timer
    let core = arch::core_id() & 3;
    //How to do this in Rust
    unsafe {
        ARM_GENERIC_TIMERS[core as usize].intialize_timer();
        ARM_GENERIC_TIMERS[core as usize].set_timer(0x5000000);
    }
}

fn arm_generic_timer_irq_handler(_ctx: &mut Context) {
    //Reset the timer to ping again
    let core = arch::core_id() & 3;
    unsafe {
        ARM_GENERIC_TIMERS[core as usize].reset_timer();
    }

    //Do things
}

pub fn get_time() -> u64 {
    SYSTEM_TIMER.get().get_time()
}

pub unsafe fn initialize_system_timer(base: *mut ()) {
    unsafe { SYSTEM_TIMER.init(Bcm2835SysTmr::new(base as *mut ())) };
    register_system_timer_irqs();
}

fn register_system_timer_irqs() {
    //Correct timer IRQ handlers for the Bcm2835_SystemTimer (Non functional in QEMU)
    gic_register_isr(96, system_timer_irq_handler);
    gic_register_isr(97, system_timer_irq_handler);
    gic_register_isr(98, system_timer_irq_handler);
    gic_register_isr(99, system_timer_irq_handler);
}

fn system_timer_irq_handler(_ctx: &mut Context) {
    panic!("System timer IRQ handler not implemented");
}

// Need one per core
impl ArmGenericTimer {
    pub const fn new() -> Self {
        Self {
            is_enabled: false,
            freq: 0,
            time_skip: 0,
        }
    }

    /// Initialize the timer before everything else
    pub fn intialize_timer(&mut self) {
        self.freq = read_cntfrq();
    }

    pub fn get_time(&self) -> u64 {
        read_cntpct()
    }

    //after an interrupt, starts the timer again
    pub fn reset_timer(&self) {
        write_cntp_tval(self.time_skip);
        enable_cntp();
    }

    //time is in the future
    pub fn set_timer(&mut self, time: u64) {
        self.time_skip = time;
        if !self.is_enabled {
            write_cntp_tval(time);
            enable_cntp();
            self.is_enabled = true;
        } else {
            write_cntp_cval(time);
        }
    }

    pub fn set_timer_seconds(&mut self, seconds: u64) {
        let time = self.get_time() + (seconds * self.freq);
        self.set_timer(time);
    }

    pub fn set_timer_milliseconds(&mut self, milliseconds: u64) {
        let time = self.get_time() + (milliseconds * self.freq / 1000);
        self.set_timer(time);
    }

    pub fn set_timer_microseconds(&mut self, microseconds: u64) {
        let time = self.get_time() + (microseconds * self.freq / 1_000_000);
        self.set_timer(time);
    }

    pub fn clear_timer(&mut self) {
        disable_cntp();
        self.is_enabled = false;
    }
}

#[allow(dead_code)]
#[rustfmt::skip]
impl Bcm2835SysTmr {
    const TIMER_CS: usize = 0x00;   //System Timer Control/Status
    const TIMER_CLO: usize = 0x04;  //Current System Timer Lower
    const TIMER_CHI: usize = 0x08;  //Current System Timer Higher
    const TIMER_C0: usize = 0x0C;   //System Timer Compare 0
    const TIMER_C1: usize = 0x10;   //System Timer Compare 1
    const TIMER_C2: usize = 0x14;   //System Timer Compare 2
    const TIMER_C3: usize = 0x18;   //System Timer Compare 3
}

impl Bcm2835SysTmr {
    pub unsafe fn new(base: *mut ()) -> Self {
        Self {
            base: base as usize,
        }
    }

    pub fn get_time(&self) -> u64 {
        let mut hi: u32;
        let mut lo: u32;

        loop {
            hi = unsafe { self.reg(Self::TIMER_CHI).read() };
            lo = unsafe { self.reg(Self::TIMER_CLO).read() };
            if hi == unsafe { self.reg(Self::TIMER_CHI).read() } {
                break;
            }
        }
        // unsafe { self.reg(Self::TIMER_C3).write(lo + 0x100) };
        ((hi as u64) << 32) | (lo as u64)
    }

    /// Set a timer after a set time on 4 available channels
    /// Currently not functional in QEMU
    pub fn set_system_timer(&self, channel: u8, time: u32) {
        let channel_adder = match channel {
            0 => Self::TIMER_C0,
            1 => Self::TIMER_C1,
            2 => Self::TIMER_C2,
            3 => Self::TIMER_C3,
            _ => panic!("Invalid channel"),
        };

        unsafe { self.reg(channel_adder).write(time) };
    }

    /// Clear the pending interrupt for a channel
    /// Currently not functional in QEMU
    pub fn clear_system_interrupt(&self, channel: u8) {
        unsafe { self.reg(Self::TIMER_CS).write(1 << channel) };
    }

    fn reg(&self, reg: usize) -> Volatile<u32> {
        Volatile((self.base as *mut u32).wrapping_byte_add(reg).cast::<u32>())
    }
}

pub struct Bcm2835SysTmr {
    base: usize,
}

pub struct ArmGenericTimer {
    is_enabled: bool,
    freq: u64,
    time_skip: u64,
}

#[inline(always)]
fn read_cntpct() -> u64 {
    let cnt: u64;
    unsafe { asm!("mrs {}, cntpct_el0", out(reg) cnt, options(nomem, nostack, preserves_flags)) };
    cnt
}

#[inline(always)]
fn read_cntfrq() -> u64 {
    let cnt: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) cnt, options(nomem, nostack, preserves_flags)) };
    cnt
}

#[inline(always)]
fn write_cntp_cval(val: u64) {
    unsafe { asm!("msr cntp_cval_el0, {}", in(reg) val, options(nomem, nostack, preserves_flags)) };
}

#[inline(always)]
fn write_cntp_tval(val: u64) {
    unsafe { asm!("msr cntp_tval_el0, {}", in(reg) val, options(nomem, nostack, preserves_flags)) };
}

#[inline(always)]
fn enable_cntp() {
    let ctl: u64 = 1;
    unsafe { asm!("msr cntp_ctl_el0, {}", in(reg) ctl, options(nomem, nostack, preserves_flags)) };
}

#[inline(always)]
fn disable_cntp() {
    let ctl: u64 = 0;
    unsafe { asm!("msr cntp_ctl_el0, {}", in(reg) ctl, options(nomem, nostack, preserves_flags)) };
}
