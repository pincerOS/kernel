#![allow(dead_code, nonstandard_style)]

//! A driver for bcm2835-wdt, the power watchdog on RPi
//!
//! The base address is discoverable from the device tree,
//! but the interface and registers are only documented in
//! the Linux driver.
//!
//! ## Brief documentation:
//!
//! Three relevant memory mapped registers:
//!
//! | register  | offset   | type  | value |
//! | --------- | -------- | ----- | ----- |
//! | `PM_RSTC` | `0x001C` | `u32` | control register |
//! | `PM_RSTS` | `0x0020` | `u32` | partition to boot from |
//! | `PM_WDOG` | `0x0024` | `u32` | remaining watchdog time |
//!
//! The base address of the mmio registers can be found through
//! the device tree, at /soc/watchdog; on RPi3, the base address
//! is `0x3F100000`.  (Note that the device tree address must be
//! translated, and is initially `0x7E100000`.)
//!
//! Each register is 32 bits, and writes to the registers must
//! have a password value of `0x5A` in the top byte.
//!
//! ### `PM_RSTC` register
//!
//! `PM_RSTC` controls what action the watchdog should take when the
//! timer is up. Two values used in Linux:
//! * `WRCFG_FULL_RESET` (`0x0020`): reset the CPU when triggered
//! * `RESET` (`0x0102`): disable the watchdog
//!
//! https://github.com/raspberrypi/linux/issues/932#issuecomment-93989581
//! - bits 31-24 `PASSWD`: Power Manager password, `0x5A` (W)
//! - bits 23-13 unused (R)
//! - bits 21-20 `HRCFG`: Hard reset configuration (R/W)
//!     - Not used on BCM2708A0.
//! - bits 19-18 unused (R)
//! - bits 17-16 `FRCFG`: Full reset configuration (R/W)
//!     - Not used on BCM2708A0.
//! - bits 15-14 unused (R)
//! - bits 13-12 `QRCFG`: Quick reset configuration (R/W)
//!     - `0b00` = do not reset PLLs
//!     - `0b01` = reset PLLs
//!     - `0b10` = do not reset PLLs
//!     - `0b11` = reset PLLs
//! - bits 11-10 unused (R)
//! - bits 9-8 `SRCFG`: Software reset configuration (R/W)
//! - bits 7-6 unused (R)
//! - bits 5-4 `WRCFG`: Watchdog reset configuration (R/W)
//! - bits 3-2 unused (R)
//! - bits 1-0 `DRCFG`: Debugger reset configuration (R/W)
//!
//! `SRCFG`, `WRCFG`, and `DRCFG` have the following values:
//! - `0b00` = no reset
//! - `0b01` = quick reset
//! - `0b10` = full reset
//! - `0b11` = hard reset
//!
//! ### `PM_RSTS` register
//!
//! `PM_RSTS` indicates what type of reset last occurred, and
//! also controls which partition the bootloader should boot from
//! after reset.
//!
//! The partition value is split across bits 0, 2, 4, 6, 8, and 10.
//! If all partition bits are set (a value of 63), the bootloader
//! will not boot and will leave the system in a low-power state.
//!
//! https://github.com/raspberrypi/linux/issues/932#issuecomment-93989581
//! - bits 31-24 `PASSWD`: Power Manager password, `0x5A` (W)
//! - bits 23-13 unused (R)
//! - bit 12 `HADPOR`: Had a power-on reset (R/W)
//! - bit 11 unused (R)
//! - bit 10 `HADSRH`: Had a software hard reset (R/W)
//! - bit  9 `HADSRF`: Had a software full reset (R/W)
//! - bit  8 `HADSRQ`: Had a software quick reset (R/W)
//! - bit  7 unused (R)
//! - bit  6 `HADWRH`: Had a watchdog hard reset (R/W)
//! - bit  5 `HADWRF`: Had a watchdog full reset (R/W)
//! - bit  4 `HADWRQ`: Had a watchdog quick reset (R/W)
//! - bit  3 unused (R)
//! - bit  2 `HADDRH`: Had a debugger hard reset (R/W)
//! - bit  1 `HADDRF`: Had a debugger full reset (R/W)
//! - bit  0 `HADDRQ`: Had a debugger quick reset (R/W)
//!
//! Each flag may be cleared by writing a zero to it.
//!
//! ### `PM_WDOG` register
//!
//! `PM_WDOG` contains the time until the watchdog triggers.
//!
//! The timer value is stored in the low 20 bits of the register;
//! the clock ticks at 2^16 Hz. (unconfirmed)

use crate::sync::Volatile;

const PM_RSTC: usize = 0x1c;
const PM_RSTS: usize = 0x20;
const PM_WDOG: usize = 0x24;

const PM_PASSWORD: u32 = 0x5a000000;
const PM_PASSWORD_MASK: u32 = 0xFF000000;

// Software: quick reset, watchdog: none, debugger: full reset
const PM_RSTC_RESET: u32 = 0x00000102;
// Software: none, watchdog: full reset, debugger: none
const PM_RSTC_WRCFG_FULL_RESET: u32 = 0x00000020;
const PM_RSTC_WRCFG_MASK: u32 = 0x00000030;

const PM_RSTS_PARTITION_MASK: u32 = 0x00000555;

const PM_WDOG_TIME_MASK: u32 = 0x000fffff;

pub struct bcm2835_wdt_driver {
    base_addr: *mut (),
    last_reset: u32,
}

impl bcm2835_wdt_driver {
    pub unsafe fn init(base_addr: *mut ()) -> Self {
        let mut driver = bcm2835_wdt_driver {
            base_addr,
            last_reset: 0,
        };
        driver.last_reset = unsafe { driver.read_last_reset() };
        driver
    }

    unsafe fn read_last_reset(&self) -> u32 {
        // TODO: check the mailbox approach to finding this? `vcgencmd get_rsts`
        // This likely has the cached version from the original boot, as this will
        // be overidden when the watchdog is enabled.
        // https://github.com/raspberrypi/utils/blob/master/vcgencmd/vcgencmd.c
        unsafe {
            let reg_rsts = self.base_addr.byte_add(PM_RSTS).cast::<u32>();
            reg_rsts.read()
        }
    }

    fn reg_rstc(&self) -> Volatile<u32> {
        Volatile(self.base_addr.wrapping_byte_add(PM_RSTC).cast::<u32>())
    }
    fn reg_rsts(&self) -> Volatile<u32> {
        Volatile(self.base_addr.wrapping_byte_add(PM_RSTS).cast::<u32>())
    }
    fn reg_wdog(&self) -> Volatile<u32> {
        Volatile(self.base_addr.wrapping_byte_add(PM_WDOG).cast::<u32>())
    }

    pub fn last_reset(&self) -> u32 {
        self.last_reset
    }

    pub unsafe fn reset(&mut self, partition: u8) {
        // The RSTS register contains the partition the bootloader will boot from
        // on reset; if it is 63, the bootloader will not boot, and will keep
        // the device in a relatively low power state (check?)

        // TODO: check whether the partition indicator is actually real.
        // Linux uses it, so the bootloader likely respects it, but this
        // appears to just be re-using existing registers as a data-channel...

        // Other sources just set the hard reset bit. (HADWDHR)

        let partition = partition as u32;
        let partition_bits = (((partition & 0b000001) >> 0) << 0)
            | (((partition & 0b000010) >> 1) << 2)
            | (((partition & 0b000100) >> 2) << 4)
            | (((partition & 0b001000) >> 3) << 6)
            | (((partition & 0b010000) >> 4) << 8)
            | (((partition & 0b100000) >> 5) << 10);

        // Set a 10-tick timeout (supposedly 150µs)
        // (Clock frequency is approx 2^16 Hz?)
        let ticks = 10;

        let reg_rstc = self.reg_rstc();
        let reg_rsts = self.reg_rsts();
        let reg_wdog = self.reg_wdog();
        unsafe {
            let mut rsts = reg_rsts.read();
            rsts &= !PM_PASSWORD_MASK | !PM_RSTS_PARTITION_MASK;
            rsts |= PM_PASSWORD | partition_bits;
            reg_rsts.write(rsts);

            let wdog = PM_PASSWORD | (PM_WDOG_TIME_MASK & ticks);
            reg_wdog.write(wdog);

            let mut rstc = reg_rstc.read();
            rstc &= !PM_PASSWORD_MASK | !PM_RSTC_WRCFG_MASK;
            rstc |= PM_PASSWORD | PM_RSTC_WRCFG_FULL_RESET;
            reg_rstc.write(rstc);
        }
    }

    pub unsafe fn set_timeout(&mut self, ticks: u32) {
        assert!(ticks <= PM_WDOG_TIME_MASK);
        let reg_rstc = self.reg_rstc();
        let reg_wdog = self.reg_wdog();
        unsafe {
            let wdog = PM_PASSWORD | (PM_WDOG_TIME_MASK & ticks);
            reg_wdog.write(wdog);

            let mut rstc = reg_rstc.read();
            rstc &= !PM_PASSWORD_MASK | !PM_RSTC_WRCFG_MASK;
            rstc |= PM_PASSWORD | PM_RSTC_WRCFG_FULL_RESET;
            reg_rstc.write(rstc);
        }
    }

    pub unsafe fn clear_timeout(&mut self) {
        let reg_rstc = self.reg_rstc();
        unsafe {
            let mut rstc = reg_rstc.read();
            rstc &= !PM_PASSWORD_MASK | !PM_RSTC_WRCFG_MASK;
            reg_rstc.write(rstc);
        }
    }

    pub unsafe fn remaining_ticks(&self) -> u32 {
        let reg_wdog = self.reg_wdog();
        unsafe { reg_wdog.read() & PM_WDOG_TIME_MASK }
    }

    pub unsafe fn timeout_active(&self) -> bool {
        let reg_rstc = self.reg_rstc();
        unsafe { (reg_rstc.read() & PM_RSTC_WRCFG_FULL_RESET) != 0 }
    }
}

unsafe impl Send for bcm2835_wdt_driver {}
unsafe impl Sync for bcm2835_wdt_driver {}
