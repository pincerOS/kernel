#![allow(dead_code, nonstandard_style)]

// https://elinux.org/BCM2835_registers#RNG
// https://github.com/raspberrypi/linux/blob/204050d0eafb565b68abf512710036c10ef1bd23/drivers/char/hw_random/bcm2835-rng.c
// https://github.com/torvalds/linux/blob/master/drivers/char/hw_random/bcm2835-rng.c

// RNG driver for the BCM2385
// Will not work for the Raspberry Pi 4b

use crate::sync::Volatile;

const RNG_CTRL: usize = 0x00;
const RNG_STATUS: usize = 0x04;
const RNG_DATA: usize = 0x08;
const RNG_FF_THRESH: usize = 0x0C; //unused
const RNG_INT_MASK: usize = 0x10;

const RNG_RBGEN: u32 = 0x1;

const RNG_WARMPUP_COUNT: u32 = 0x40000;

const RNG_INT_OFF: u32 = 0x1;

pub struct bcm2835_rng_driver {
    base_addr: *mut (),
}

impl bcm2835_rng_driver {
    pub unsafe fn init(base_addr: *mut ()) -> Self {
        let driver = bcm2835_rng_driver { base_addr };

        let reg_ctrl = driver.reg_ctrl();
        let reg_int_mask = driver.reg_int_mask();
        let reg_status = driver.reg_status();

        unsafe {
            // Mask the interrupt, not sure if needed or not
            let mut val = reg_int_mask.read();
            val |= RNG_INT_OFF;
            reg_int_mask.write(val);

            reg_status.write(RNG_WARMPUP_COUNT);
            reg_ctrl.write(RNG_RBGEN);
        };

        driver
    }

    fn reg_ctrl(&self) -> Volatile<u32> {
        Volatile(self.base_addr.wrapping_byte_add(RNG_CTRL).cast::<u32>())
    }

    fn reg_status(&self) -> Volatile<u32> {
        Volatile(self.base_addr.wrapping_byte_add(RNG_STATUS).cast::<u32>())
    }

    fn reg_data(&self) -> Volatile<u32> {
        Volatile(self.base_addr.wrapping_byte_add(RNG_DATA).cast::<u32>())
    }

    fn reg_int_mask(&self) -> Volatile<u32> {
        Volatile(self.base_addr.wrapping_byte_add(RNG_INT_MASK).cast::<u32>())
    }

    pub fn rng_read(&mut self, buf: &mut [u32], wait: bool) -> usize {
        let reg_status = self.reg_status();
        let reg_data = self.reg_data();

        unsafe {
            while (reg_status.read() >> 24) == 0 {
                //until word is ready to be read
                if !wait {
                    return 0;
                }
            }

            let mut num_words = reg_status.read() >> 24;
            if num_words as usize > buf.len() {
                num_words = buf.len() as u32;
            }

            for i in 0..num_words as usize {
                buf[i] = reg_data.read();
            }
            num_words as usize
        }
    }
}
