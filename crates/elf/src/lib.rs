#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod elf;
pub use elf::*;
