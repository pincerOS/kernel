#![allow(dead_code, nonstandard_style)]

// https://github.com/PythonHacker24/rpi-gpio-driver/blob/main/gpio_driver.c
// https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf

use crate::sync::Volatile;

/// GPIO Register Offsets
const GPFSEL0: usize = 0x00; // Function Select
const GPSET0: usize = 0x1C; // Output Set
const GPCLR0: usize = 0x28; // Output Clear
const GPLEV0: usize = 0x34; // Pin Level
const GPEDS0: usize = 0x40; // Event Detect Status
const GPREN0: usize = 0x4C; // Rising Edge Detect
const GPFEN0: usize = 0x58; // Falling Edge Detect
const GPHEN0: usize = 0x64; // Pin High Detect
const GPLEN0: usize = 0x70; // Pin Low Detect
const GPAREN0: usize = 0x7c; // Async. Rising Edge Detect
const GPAFEN0: usize = 0x88; // Async. Falling Edge Detect
const GPIO_PUP_PDN_CNTRL_REG0: usize = 0xE4; // Pull-up/down Enable

/// GPIO Function Select values
#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum GpioFunction {
    Input = 0b000,
    Output = 0b001,
    Alt0 = 0b100,
    Alt1 = 0b101,
    Alt2 = 0b110,
    Alt3 = 0b111,
    Alt4 = 0b011,
    Alt5 = 0b010,
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum GpioPull {
    None = 0b00,
    PullUp = 0b01,
    PullDown = 0b10,
}

#[derive(Copy, Clone)]
pub enum GpioEvent {
    RISING_EDGE,
    FALLING_EDGE,
    HIGH,
    LOW,
    ASYNC_RISING_EDGE,
    ASYNC_FALLING_EDGE,
}

pub struct bcm2711_gpio_driver {
    base_addr: *mut (),
}

impl bcm2711_gpio_driver {
    pub unsafe fn init(base_addr: *mut ()) -> Self {
        Self { base_addr }
    }

    pub unsafe fn init_with_defaults(base_addr: *mut (), apply_defaults: bool) -> Self {
        let mut driver = bcm2711_gpio_driver { base_addr };

        if apply_defaults {
            // Set UART pins
            driver.set_function(14, GpioFunction::Alt0); // UART TX
            driver.set_function(15, GpioFunction::Alt0); // UART RX

            // Add more defaults
        }

        driver
    }

    fn reg_fsel(&self, index: usize) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(GPFSEL0 + (index * 4))
                .cast::<u32>(),
        )
    }

    fn reg_set(&self, index: usize) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(GPSET0 + (index * 4))
                .cast::<u32>(),
        )
    }

    fn reg_clr(&self, index: usize) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(GPCLR0 + (index * 4))
                .cast::<u32>(),
        )
    }

    fn reg_lev(&self, index: usize) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(GPLEV0 + (index * 4))
                .cast::<u32>(),
        )
    }

    fn reg_pup_pdn(&self, index: usize) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(GPIO_PUP_PDN_CNTRL_REG0 + (index * 4))
                .cast::<u32>(),
        )
    }

    /// Add event detect for a GPIO pin.
    /// When event happens on the specified pin, a 1 gets written to the relevant
    /// GPPEDSn register and an interrupt is triggered.
    /// Pins 0-27 triggers IRQ 49 (GPIO 0).
    /// Pins 28-45 triggers IRQ 50 (GPIO 1).
    /// Pins 46-57 triggers IRQ 51 (GPIO 2).
    /// IRQ 52 (GPIO 3) is triggered whenever any bit is set.
    pub fn add_event_detect(&mut self, pin: u8, event: GpioEvent) {
        let index = (pin / 32) as usize;
        let shift = pin % 32;
        let reg_offset = match event {
            GpioEvent::RISING_EDGE => GPREN0,
            GpioEvent::FALLING_EDGE => GPFEN0,
            GpioEvent::HIGH => GPHEN0,
            GpioEvent::LOW => GPLEN0,
            GpioEvent::ASYNC_RISING_EDGE => GPAREN0,
            GpioEvent::ASYNC_FALLING_EDGE => GPAFEN0,
        };
        let reg = Volatile(
            self.base_addr
                .wrapping_byte_add(reg_offset + (index * 4))
                .cast::<u32>(),
        );
        unsafe {
            reg.write(reg.read() | (1 << shift));
        }
    }

    /// Set the function of a GPIO pin
    pub fn set_function(&mut self, pin: u8, function: GpioFunction) {
        let index = (pin / 10) as usize;
        let shift = (pin % 10) * 3;
        let reg_fsel = self.reg_fsel(index);

        unsafe {
            let mut val = reg_fsel.read();
            val &= !(0b111 << shift); // Clear current function bits
            val |= (function as u32) << shift;
            reg_fsel.write(val);
            // #[cfg(debug_assertions)]
            // print!("| GPIO -- Wrote {:#010b}, to register GPFSEL{index}, function: {:?} for pin {pin}\n", val, function);
        }
    }

    /// Set a GPIO pin HIGH
    pub fn set_high(&mut self, pin: u8) {
        let index = (pin / 32) as usize;
        let shift = pin % 32;
        let reg_set = self.reg_set(index);

        unsafe {
            reg_set.write(1 << shift);
        }
        // #[cfg(debug_assertions)]
        // println!("| GPIO -- Set HIGH: pin {}", pin);
    }

    /// Set a GPIO pin LOW
    pub fn set_low(&mut self, pin: u8) {
        let index = (pin / 32) as usize;
        let shift = pin % 32;
        let reg_clr = self.reg_clr(index);

        unsafe {
            reg_clr.write(1 << shift);
        }
        // #[cfg(debug_assertions)]
        // println!("| GPIO -- Set LOW: pin {}", pin);
    }

    /// Read a GPIO pin value
    pub fn read(&self, pin: u8) -> bool {
        let index = (pin / 32) as usize;
        let shift = pin % 32;
        let reg_lev = self.reg_lev(index);

        unsafe { (reg_lev.read() & (1 << shift)) != 0 }
    }

    /// Read a mask of 32 pins
    pub fn read_mask(&self, index: usize) -> u32 {
        let reg_lev = self.reg_lev(index);

        unsafe { reg_lev.read() }
    }

    /// Toggle a GPIO pin
    pub fn toggle(&mut self, pin: u8) {
        if self.read(pin) {
            self.set_low(pin);
        } else {
            self.set_high(pin);
        }
    }

    /// Configure pull-up/down resistor
    pub fn set_pull(&mut self, pin: u8, pull: GpioPull) {
        let index = (pin / 16) as usize;
        let shift = (pin % 16) * 2;
        let reg_pup_pdn = self.reg_pup_pdn(index);
        unsafe {
            let mut val = reg_pup_pdn.read();
            val &= !(0b11 << shift); // Clear the 2-bit field
            val |= (pull as u32) << shift; // Set the new pull mode
            reg_pup_pdn.write(val);
        }
    }

    /// Configure pull-up/down resistor for multiple pins
    pub fn set_pull_mask(&mut self, mask: u16, index: usize, pull: GpioPull) {
        debug_assert!(index < 4);
        let reg_pup_pdn = self.reg_pup_pdn(index);
        unsafe {
            let mut val = reg_pup_pdn.read();
            for bit in 0..16 {
                if (mask & (1 << bit)) != 0 {
                    let shift = bit * 2;
                    val &= !(0b11 << shift);
                    val |= (pull as u32) << shift;
                }
            }
            reg_pup_pdn.write(val);
        }
    }

    /// Check if an event was triggered for a GPIO pin
    pub fn check_event(&self, pin: u8) -> bool {
        let index = (pin / 32) as usize;
        let shift = pin % 32;
        let reg_eds = Volatile(
            self.base_addr
                .wrapping_byte_add(GPEDS0 + (index * 4))
                .cast::<u32>(),
        );

        unsafe { (reg_eds.read() & (1 << shift)) != 0 }
    }

    /// Clear an event in GPEDS (write 1 to clear)
    pub fn clear_event(&mut self, pin: u8) {
        let index = (pin / 32) as usize;
        let shift = pin % 32;
        let reg_eds = Volatile(
            self.base_addr
                .wrapping_byte_add(GPEDS0 + (index * 4))
                .cast::<u32>(),
        );

        unsafe {
            reg_eds.write(1 << shift);
        }
    }

    /// Check if any GPIO event was triggered
    pub fn check_any_event(&self) -> bool {
        for i in 0..2 {
            let reg_eds = Volatile(
                self.base_addr
                    .wrapping_byte_add(GPEDS0 + (i * 4))
                    .cast::<u32>(),
            );
            unsafe {
                if reg_eds.read() != 0 {
                    return true;
                }
            }
        }
        false
    }

    /// Clear all GPIO events in GPEDS0 and GPEDS1
    pub fn clear_all_events(&mut self) {
        for i in 0..2 {
            let reg_eds = Volatile(
                self.base_addr
                    .wrapping_byte_add(GPEDS0 + (i * 4))
                    .cast::<u32>(),
            );
            unsafe {
                reg_eds.write(0xFFFFFFFF);
            }
        }
    }
}

unsafe impl Send for bcm2711_gpio_driver {}
unsafe impl Sync for bcm2711_gpio_driver {}
