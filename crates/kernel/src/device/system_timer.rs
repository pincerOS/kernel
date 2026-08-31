//! A system timer driver for the raspi4b.
//! This is the Bcm2835-system-timer
//!
//! The bcm2835-system-timer was implemented as the primary timer instead of the ARM Generic Timer
//!     as the BCM2711 Peripherals Manual states that the system timer is more accurate (and the arm generic timer is based off of it)
//!     and therefore the system timer should be used for time keeping
//!
//! Arm Generic Timer is implemented for each core to handle the timer interrupts, as the system timer is not functional in QEMU
//!
use super::usb::usbd::device::UsbDevice;
use super::usb::usbd::endpoint::endpoint_descriptor;
use crate::event::context::Context;
use crate::event::timer_handler;
use crate::sync::spin_sleep;
use crate::sync::{ConstInit, PerCore, UnsafeInit, Volatile};
use alloc::vec::Vec;
use core::arch::asm;
use core::panic;

pub static SYSTEM_TIMER: UnsafeInit<Bcm2835SysTmr> = unsafe { UnsafeInit::uninit() };

pub static ARM_GENERIC_TIMERS: PerCore<ArmGenericTimer> = PerCore::new();

pub static TIMER_SCHEDULER: PerCore<TimerScheduler> = PerCore::new();

unsafe impl Sync for UsbDevice {}

pub fn get_time() -> u64 {
    SYSTEM_TIMER.get().get_time()
}

pub fn micro_delay(delay: u32) {
    // let stop = get_time() + delay as u64;
    // while get_time() < stop {
    //     unsafe { asm!("nop") };
    // }
    spin_sleep(delay as usize);
}

pub unsafe fn initialize_system_timer(base: *mut ()) {
    unsafe { SYSTEM_TIMER.init(Bcm2835SysTmr::new(base)) };
    if super::gic::GIC.is_initialized() {
        // TODO: support local timer on rpi3b interrupt controller
        let gic = super::gic::GIC.get();
        register_system_timer_irqs(gic);
    }
}

fn register_system_timer_irqs(gic: &super::gic::Gic400Driver) {
    //Correct timer IRQ handlers for the Bcm2835_SystemTimer (Non functional in QEMU)
    gic.register_isr(96, system_timer_irq_handler);
    gic.register_isr(97, system_timer_irq_handler);
    gic.register_isr(98, system_timer_irq_handler);
    gic.register_isr(99, system_timer_irq_handler);
}

fn system_timer_irq_handler(_ctx: &mut Context) {
    panic!("System timer IRQ handler not implemented");
}

#[derive(PartialEq)]
enum TimerState {
    Uninit,
    Disabled,
    Enabled,
}

impl ConstInit for ArmGenericTimer {
    const INIT: Self = ArmGenericTimer::new();
}

impl ConstInit for TimerScheduler {
    const INIT: Self = TimerScheduler::new();
}

// Need one per core
impl ArmGenericTimer {
    pub const fn new() -> Self {
        Self {
            timer_state: TimerState::Uninit,
            freq: 0,
            time_skip: 0,
        }
    }

    /// Initialize the timer before everything else
    pub fn intialize_timer(&mut self) {
        unsafe { self.freq = read_cntfrq() };
        self.timer_state = TimerState::Disabled;
    }

    pub fn get_time(&self) -> u64 {
        unsafe { read_cntpct() }
    }

    //after an interrupt, starts the timer again
    pub fn reset_timer(&self) {
        match self.timer_state {
            TimerState::Uninit | TimerState::Disabled => {
                println!("Reset Timer: Timer not initialized")
            }
            TimerState::Enabled => unsafe {
                write_cntp_tval(self.time_skip);
            },
        }
    }

    //time is in the future
    pub fn set_timer(&mut self, time: u64) {
        self.time_skip = time;
        match self.timer_state {
            TimerState::Uninit => {
                //TODO: Error handling
                println!("Set Timer: Timer not initialized")
            }
            TimerState::Disabled => {
                self.timer_state = TimerState::Enabled;
                unsafe {
                    write_cntp_tval(time);
                    enable_cntp();
                }
            }
            TimerState::Enabled => {
                unsafe { write_cntp_tval(time) };
            }
        }
    }

    pub fn set_timer_abs(&mut self, time: u64) {
        match self.timer_state {
            TimerState::Uninit => {
                //TODO: Error handling
                println!("Set Timer: Timer not initialized")
            }
            TimerState::Disabled => {
                self.timer_state = TimerState::Enabled;
                unsafe {
                    write_cntp_cval(time);
                    enable_cntp();
                }
            }
            TimerState::Enabled => {
                unsafe { write_cntp_cval(time) };
            }
        }
    }

    pub fn set_timer_seconds(&mut self, seconds: u64) {
        let time = seconds * self.freq;
        self.set_timer(time);
    }

    pub fn set_timer_milliseconds(&mut self, milliseconds: u64) {
        let time = milliseconds * self.freq / 1000;
        self.set_timer(time);
    }

    pub fn set_timer_microseconds(&mut self, microseconds: u64) {
        let time = microseconds * self.freq / 1_000_000;
        self.set_timer(time);
    }

    pub fn clear_timer(&mut self) {
        unsafe { disable_cntp() };
        self.timer_state = TimerState::Disabled;
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

pub fn timer_scheduler_handler(ctx: &mut Context) {
    let time = ARM_GENERIC_TIMERS.with_current(|timer| timer.get_time());
    let mut preemption_flag = false;
    TIMER_SCHEDULER.with_current(|timer_scheduler| {
        let timer_freq = timer_scheduler.timer_freq;
        let mut timer_min = u64::MAX;
        let events = &mut timer_scheduler.timer_events;

        for event in events.iter_mut() {
            if event.timer_timer <= time {
                event.timer_timer = time + (event.repeat_time as u64) * timer_freq / 1000;
                if event.is_preempted {
                    preemption_flag = true;
                }
                (event.callback)(event.endpoint);
            }
        }
        for event in events.iter() {
            if event.timer_timer < timer_min {
                timer_min = event.timer_timer;
            }
        }
        timer_scheduler.min_time = timer_min;
        ARM_GENERIC_TIMERS.with_current(|timer| {
            timer.set_timer_abs(timer_min);
        });
    });

    if preemption_flag {
        unsafe { timer_handler(ctx) };
    }
}

pub fn timer_scheduler_add_timer_event(
    time: u32,
    callback: fn(endpoint_descriptor),
    endpoint: endpoint_descriptor,
) {
    TIMER_SCHEDULER.with_current(|timer_scheduler| {
        timer_scheduler.add_timer_event(time, callback, endpoint, false)
    });
}

impl TimerScheduler {
    pub const fn new() -> Self {
        Self {
            timer_state: TimerState::Uninit,
            timer_events: Vec::new(),
            timer_freq: 1,
            min_time: u64::MAX,
        }
    }

    pub fn intialize_timer(&mut self) {
        self.timer_freq = unsafe { read_cntfrq() };
        self.timer_state = TimerState::Enabled;

        let cur_timer = ARM_GENERIC_TIMERS.with_current(|timer| timer.get_time());
        self.min_time = u64::MAX;
        for event in self.timer_events.iter_mut() {
            event.timer_timer = cur_timer + (event.repeat_time as u64) * self.timer_freq / 1000;
            if event.timer_timer < self.min_time {
                self.min_time = event.timer_timer;
            }
        }

        ARM_GENERIC_TIMERS.with_current(|timer| {
            timer.set_timer_abs(self.min_time);
        });
    }

    //the time is the interval in milliseconds, callback is the function to call, device is the device that the timer is associated with
    pub fn add_timer_event(
        &mut self,
        time: u32,
        callback: fn(endpoint_descriptor),
        endpoint: endpoint_descriptor,
        is_preempted: bool,
    ) {
        let cur_timer = ARM_GENERIC_TIMERS.with_current(|timer| timer.get_time());
        self.timer_events.push(TimerEvent {
            timer_timer: cur_timer + (time as u64) * self.timer_freq / 1000,
            repeat_time: time,
            callback,
            endpoint: endpoint,
            is_preempted: is_preempted,
        });
        for event in self.timer_events.iter() {
            if event.timer_timer < self.min_time {
                self.min_time = event.timer_timer;
            }
        }

        if self.timer_state == TimerState::Enabled {
            ARM_GENERIC_TIMERS.with_current(|timer| {
                timer.set_timer_abs(self.min_time);
            });
        }
    }
}

unsafe impl Sync for TimerEvent {}

pub struct TimerEvent {
    timer_timer: u64,
    repeat_time: u32,
    callback: fn(endpoint_descriptor),
    endpoint: endpoint_descriptor,
    is_preempted: bool, //TODO: Hacky solution as of now, it just checks if there is a preemption flag, and will preempt after running
}

pub struct TimerScheduler {
    timer_state: TimerState,
    timer_events: Vec<TimerEvent>,
    timer_freq: u64,
    min_time: u64,
}

unsafe impl Send for TimerScheduler {}
unsafe impl Sync for TimerScheduler {}

pub struct Bcm2835SysTmr {
    base: usize,
}

pub struct ArmGenericTimer {
    timer_state: TimerState,
    freq: u64,
    time_skip: u64,
}

#[inline(always)]
unsafe fn read_cntpct() -> u64 {
    let cnt: u64;
    unsafe { asm!("mrs {}, cntpct_el0", out(reg) cnt, options(nomem, nostack, preserves_flags)) };
    cnt
}

#[inline(always)]
unsafe fn read_cntfrq() -> u64 {
    let cnt: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) cnt, options(nomem, nostack, preserves_flags)) };
    cnt
}

#[allow(dead_code)]
#[inline(always)]
unsafe fn write_cntp_cval(val: u64) {
    unsafe { asm!("msr cntp_cval_el0, {}", in(reg) val, options(nomem, nostack, preserves_flags)) };
}

#[inline(always)]
unsafe fn write_cntp_tval(val: u64) {
    unsafe { asm!("msr cntp_tval_el0, {}", in(reg) val, options(nomem, nostack, preserves_flags)) };
}

#[inline(always)]
unsafe fn enable_cntp() {
    let ctl: u64 = 1;
    unsafe { asm!("msr cntp_ctl_el0, {}", in(reg) ctl, options(nomem, nostack, preserves_flags)) };
}

#[inline(always)]
unsafe fn disable_cntp() {
    let ctl: u64 = 0;
    unsafe { asm!("msr cntp_ctl_el0, {}", in(reg) ctl, options(nomem, nostack, preserves_flags)) };
}
