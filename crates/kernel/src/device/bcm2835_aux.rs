use crate::sync::{SpinLock, UnsafeInit,Volatile};

use super::GPIO;

// https://www.raspberrypi.org/app/uploads/2012/02/BCM2835-ARM-Peripherals.pdf

pub static MINI_UART: UnsafeInit<SpinLock<MiniUart>> = unsafe { UnsafeInit::uninit() };


pub struct MiniUart {
    base: *mut (),
}

#[allow(dead_code)]
#[rustfmt::skip]
impl MiniUart {
    const AUX_IRQ: usize = 0x0000;            // Auxiliary Interrupt status 3
    const AUX_ENABLES: usize = 0x0004;        // Auxiliary enables 3
    const AUX_MU_IO_REG: usize = 0x0040;      // Mini Uart I/O Data 8
    const AUX_MU_IER_REG: usize = 0x0044;     // Mini Uart Interrupt Enable 8
    const AUX_MU_IIR_REG: usize = 0x0048;     // Mini Uart Interrupt Identify 8
    const AUX_MU_LCR_REG: usize = 0x004C;     // Mini Uart Line Control 8
    const AUX_MU_MCR_REG: usize = 0x0050;     // Mini Uart Modem Control 8
    const AUX_MU_LSR_REG: usize = 0x0054;     // Mini Uart Line Status 8
    const AUX_MU_MSR_REG: usize = 0x0058;     // Mini Uart Modem Status 8
    const AUX_MU_SCRATCH: usize = 0x005C;     // Mini Uart Scratch 8
    const AUX_MU_CNTL_REG: usize = 0x0060;    // Mini Uart Extra Control 8
    const AUX_MU_STAT_REG: usize = 0x0064;    // Mini Uart Extra Status 32
    const AUX_MU_BAUD_REG: usize = 0x0068;    // Mini Uart Baudrate 16
    const AUX_SPI0_CNTL0_REG: usize = 0x0080; // SPI 1 Control register 0 32
    const AUX_SPI0_CNTL1_REG: usize = 0x0084; // SPI 1 Control register 1 8
    const AUX_SPI0_STAT_REG: usize = 0x0088;  // SPI 1 Status 32
    const AUX_SPI0_IO_REG: usize = 0x0090;    // SPI 1 Data 32
    const AUX_SPI0_PEEK_REG: usize = 0x0094;  // SPI 1 Peek 16
    const AUX_SPI1_CNTL0_REG: usize = 0x00C0; // SPI 2 Control register 0 32
    const AUX_SPI1_CNTL1_REG: usize = 0x00C4; // SPI 2 Control register 1 8
    const AUX_SPI1_STAT_REG: usize = 0x00C8;  // SPI 2 Status 32
    const AUX_SPI1_IO_REG: usize = 0x00D0;    // SPI 2 Data 32
    const AUX_SPI1_PEEK_REG: usize = 0x00D4;  // SPI 2 Peek 16
}

impl MiniUart {
    pub unsafe fn new(base: *mut ()) -> Self {
        // TODO[hardware]: proper mini UART initialization
        // (this is enough to convince qemu that it's initialized)
        let this = Self { base };
        unsafe {
            // Enable mini UART, disable both SPI modules
            this.reg(Self::AUX_ENABLES).write(0b001);
            // Clear bit 7, DLAB (baudrate register access instead of data)
            // Clear bit 6, break

            this.reg(Self::AUX_MU_IER_REG).write(0);
            this.reg(Self::AUX_MU_CNTL_REG).write(0);
            this.reg(Self::AUX_MU_LCR_REG).write(3);
            this.reg(Self::AUX_MU_MCR_REG).write(0);
            this.reg(Self::AUX_MU_IER_REG).write(0);
            this.reg(Self::AUX_MU_IIR_REG).write(0xc6);
            this.reg(Self::AUX_MU_BAUD_REG).write((500000000 / (115200 * 8)) - 1);
            // this.reg(Self::AUX_MU_BAUD_REG).write(270);
            let mut gpio = GPIO.get().lock();
            gpio.set_function(14, super::gpio::GpioFunction::Alt5); // GPIO 14 (TXD) to alt5
            gpio.set_function(15, super::gpio::GpioFunction::Alt5); // GPIO 15 (RXD) to alt5
            gpio.set_pull(14, super::gpio::GpioPull::None);
            gpio.set_pull(15, super::gpio::GpioPull::None);
            
            this.reg(Self::AUX_MU_CNTL_REG).write(3);

            // Set bit 0, 8-bit mode
            // #[allow(clippy::eq_op)]
            // this.reg(Self::AUX_MU_LCR_REG)
            //     .write((0 << 7) | (0 << 6) | (1 << 0));

            // TODO: initialize GPIO
        }
        this
    }

    fn reg(&self, reg: usize) -> Volatile<u32> {
        Volatile(self.base.wrapping_byte_add(reg).cast::<u32>())
    }
    unsafe fn transmit_fifo_full(&self) -> bool {
        (unsafe { self.reg(Self::AUX_MU_LSR_REG).read() } & (1 << 5)) == 0
    }
    unsafe fn receive_fifo_empty(&self) -> bool {
        (unsafe { self.reg(Self::AUX_MU_LSR_REG).read() } & 0b1) == 0
    }
    pub fn writec(&mut self, c: u8) {
        unsafe {
            while self.transmit_fifo_full() {}
            self.reg(Self::AUX_MU_IO_REG).write(c as u32);
        }
    }
    pub fn getc(&mut self) -> u8 {
        unsafe {
            while self.receive_fifo_empty() {}
            self.reg(Self::AUX_MU_IO_REG).read() as u8
        }
    }
    pub fn try_getc(&mut self) -> Option<u8> {
        unsafe {
            if self.receive_fifo_empty() {
                None
            } else {
                Some(self.reg(Self::AUX_MU_IO_REG).read() as u8)
            }
        }
    }
}

unsafe impl Send for MiniUart {}

impl core::fmt::Write for MiniUart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            self.writec(b);
        }
        Ok(())
    }
}

impl core::fmt::Write for &SpinLock<MiniUart> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut inner = self.lock();
        for b in s.bytes() {
            inner.writec(b);
        }
        Ok(())
    }
    fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> core::fmt::Result {
        let mut inner = self.lock();
        inner.write_fmt(args)
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        write!($crate::device::bcm2835_aux::MINI_UART.get(), $($arg)*).ok();
    }};
}
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        writeln!($crate::device::bcm2835_aux::MINI_UART.get(), $($arg)*).ok();
    }};
}
