#![no_std]

pub mod sys;

#[cfg(feature = "runtime")]
pub mod runtime;

pub struct Stdout;

impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let msg = sys::Message {
            tag: 0,
            objects: [0; 4],
        };
        let chan = sys::ChannelDesc(1);
        sys::send_block(chan, &msg, s.as_bytes());
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        write!($crate::Stdout, $($arg)*).ok();
    }};
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        writeln!($crate::Stdout, $($arg)*).ok();
    }};
}
