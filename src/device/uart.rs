use crate::sync::{SpinLock, UnsafeInit, Volatile};

pub static UART: UnsafeInit<SpinLock<UARTInner>> = unsafe { UnsafeInit::uninit() };

pub struct UARTInner {
    base: *mut (),
}

impl UARTInner {
    const UART_DR: usize = 0x00;
    const UART_FR: usize = 0x18;

    pub fn new(base: *mut ()) -> Self {
        // TODO[hardware]: proper UART initialization
        Self { base }
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
