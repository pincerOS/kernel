use crate::sys::pwrite_all;

pub struct Stdout;

impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        pwrite_all(1, s.as_bytes(), 0)
            .map(|_| ())
            .map_err(|_| core::fmt::Error)
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        write!($crate::stdout::Stdout, $($arg)*).ok();
    }};
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        writeln!($crate::stdout::Stdout, $($arg)*).ok();
    }};
}
