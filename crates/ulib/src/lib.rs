#![no_std]

#[cfg(feature = "runtime")]
pub mod runtime;

pub mod macros;
pub mod sys;

pub struct Stdout;

impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        sys::pwrite_all(1, s.as_bytes(), 0)
            .map(|_| ())
            .map_err(|_| core::fmt::Error)
    }
}
