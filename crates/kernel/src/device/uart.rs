use crate::sync::{SpinLock, UnsafeInit, Volatile};

pub static UART: UnsafeInit<SpinLock<UARTInner>> = unsafe { UnsafeInit::uninit() };

pub struct UARTInner {
    base: *mut (),
}

impl UARTInner {
    const UART_DR: usize = 0x00;
    const UART_FR: usize = 0x18;
    const UART_IBRD: usize = 0x24;
    const UART_FBRD: usize = 0x28;
    const UART_LCRH: usize = 0x2C;
    const UART_IMSC: usize = 0x38;
    const UART_CR: usize = 0x30;
    const UART_ICR: usize = 0x44;

    pub unsafe fn new(base: *mut ()) -> Self {
        // TODO[hardware]: proper UART initialization
        // (this is enough to convince qemu that it's initialized)

        let this = Self { base };

        unsafe {
            // Disable UART0.
            this.reg(Self::UART_CR).write(0x00000000);
            // Setup the GPIO pin 14 && 15.

            // TODO: GPIO init
            // // Disable pull up/down for all GPIO pins & delay for 150 cycles.
            // this.reg(Self::GPPUD).write(0x00000000);
            // crate::sync::spin_sleep(1);

            // // Disable pull up/down for pin 14,15 & delay for 150 cycles.
            // this.reg(Self::GPPUDCLK0).write((1 << 14) | (1 << 15));
            // crate::sync::spin_sleep(1);

            // // Write 0 to GPPUDCLK0 to make it take effect.
            // this.reg(Self::GPPUDCLK0).write(0x00000000);

            // Clear pending interrupts.
            this.reg(Self::UART_ICR).write(0x7FF);

            // Set integer & fractional part of baud rate.
            // Divider = UART_CLOCK/(16 * Baud)
            // Fraction part register = (Fractional part * 64) + 0.5
            // Baud = 115200.

            // For Raspi3 and 4 the UART_CLOCK is system-clock dependent by default.
            // Set it to 3Mhz so that we can consistently set the baud rate
            // if (raspi >= 3) {
            //     // UART_CLOCK = 30000000;
            //     unsigned int r = (((unsigned int)(&mbox) & ~0xF) | 8);
            //     // wait until we can talk to the VC
            //     while ( mmio_read(MBOX_STATUS) & 0x80000000 ) { }
            //     // send our message to property channel and wait for the response
            //     this.reg(Self::MBOX_WRITE).write(r);
            //     while ( (mmio_read(MBOX_STATUS) & 0x40000000) || mmio_read(MBOX_READ) != r ) { }
            // }

            // Divider = 3000000 / (16 * 115200) = 1.627 = ~1.
            this.reg(Self::UART_IBRD).write(1);
            // Fractional part register = (.627 * 64) + 0.5 = 40.6 = ~40.
            this.reg(Self::UART_FBRD).write(40);

            // Enable FIFO & 8 bit data transmission (1 stop bit, no parity).
            this.reg(Self::UART_LCRH)
                .write((1 << 4) | (1 << 5) | (1 << 6));

            // Mask all interrupts.
            this.reg(Self::UART_IMSC).write(
                (1 << 1)
                    | (1 << 4)
                    | (1 << 5)
                    | (1 << 6)
                    | (1 << 7)
                    | (1 << 8)
                    | (1 << 9)
                    | (1 << 10),
            );

            // Enable UART, receive & transfer part of UART.
            this.reg(Self::UART_CR)
                .write((1 << 0) | (1 << 8) | (1 << 9));
        }
        this
    }

    fn reg(&self, reg: usize) -> Volatile<u32> {
        Volatile(self.base.wrapping_byte_add(reg).cast::<u32>())
    }
    unsafe fn transmit_fifo_full(&self) -> bool {
        (unsafe { self.reg(Self::UART_FR).read() } & (1 << 5) > 0)
    }
    unsafe fn receive_fifo_empty(&self) -> bool {
        (unsafe { self.reg(Self::UART_FR).read() } & (1 << 4) > 0)
    }
    pub fn writec(&mut self, c: u8) {
        unsafe {
            while self.transmit_fifo_full() {}
            self.reg(Self::UART_DR).write(c as u32);
        }
    }
    pub fn getc(&mut self) -> u8 {
        unsafe {
            while self.receive_fifo_empty() {}
            self.reg(Self::UART_DR).read() as u8
        }
    }
    pub fn try_getc(&mut self) -> Option<u8> {
        unsafe {
            if self.receive_fifo_empty() {
                None
            } else {
                Some(self.reg(Self::UART_DR).read() as u8)
            }
        }
    }
}

unsafe impl Send for UARTInner {}

impl core::fmt::Write for UARTInner {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            self.writec(b);
        }
        Ok(())
    }
}

impl core::fmt::Write for &SpinLock<UARTInner> {
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
        write!($crate::uart::UART.get(), $($arg)*).ok();
    }};
}
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        writeln!($crate::uart::UART.get(), $($arg)*).ok();
    }};
}
