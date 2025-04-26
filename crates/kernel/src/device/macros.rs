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
