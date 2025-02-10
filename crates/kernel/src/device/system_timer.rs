
/// A system timer driver for the raspi4b.
/// This is the Bcm2835-system-timer
/// 
/// The bcm2835-system-timer was implemented as the primary timer instead of the ARM Generic Timer
///     as the BCM2711 Peripherals Manual states that the system timer is more accurate (and the arm generic timer is based off of it)
///     and therefore the system timer should be used for time keeping
/// 

use crate::sync::Volatile;
use crate::context::Context;

use super::gic::gic_register_isr;

pub struct SystemTimer {
    base: *mut (),
}

#[allow(dead_code)]
#[rustfmt::skip]
impl SystemTimer {
    const TIMER_CS: usize = 0x00;   //System Timer Control/Status
    const TIMER_CLO: usize = 0x04;  //Current System Timer Lower
    const TIMER_CHI: usize = 0x08;  //Current System Timer Higher
    const TIMER_C0: usize = 0x0C;   //System Timer Compare 0
    const TIMER_C1: usize = 0x10;   //System Timer Compare 1
    const TIMER_C2: usize = 0x14;   //System Timer Compare 2
    const TIMER_C3: usize = 0x18;   //System Timer Compare 3
}

impl SystemTimer {
    pub unsafe fn new(base: *mut ()) -> Self {
        let this = Self { base };

        //Correct timer IRQ handlers
        gic_register_isr(96, Self::timer_irq_handler as fn(&mut Context));
        gic_register_isr(97, Self::timer_irq_handler as fn(&mut Context));
        gic_register_isr(98, Self::timer_irq_handler as fn(&mut Context));
        gic_register_isr(99, Self::timer_irq_handler as fn(&mut Context));

        this
    }

    pub fn get_time(&self) -> u64 {
        unsafe { self.reg(Self::TIMER_C0).write(0x10000) };
        unsafe { self.reg(Self::TIMER_C1).write(0x10000) };
        unsafe { self.reg(Self::TIMER_C2).write(0x10000) };
        unsafe { self.reg(Self::TIMER_C3).write(0x10000) };

        println!("{}: {}", 1, unsafe { self.reg(Self::TIMER_CS).read() });
        let mut hi: u32;
        let mut lo: u32;

        loop {
            hi = unsafe { self.reg(Self::TIMER_CHI).read() };
            lo = unsafe { self.reg(Self::TIMER_CLO).read() };
            if hi == unsafe { self.reg(Self::TIMER_CHI).read() } {
                break;
            }
        }
        unsafe { self.reg(Self::TIMER_C3).write(lo + 0x100) };
        println!("hi: {hi}, lo: {lo}");
        ((hi as u64) << 32) | (lo as u64)
    }

    /// Set a timer after a set time on 4 available channels
    pub fn set_timer(&self, channel: u8, time: u32) {
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
    pub fn clear_interrupt(&self, channel: u8) {
        unsafe { self.reg(Self::TIMER_CS).write(1 << channel) };
    }

    pub fn timer_irq_handler(_ctx: &mut Context) {
        println!("Timer IRQ");
    }

    fn reg(&self, reg: usize) -> Volatile<u32> {
        Volatile(self.base.wrapping_byte_add(reg).cast::<u32>())
    }
}
