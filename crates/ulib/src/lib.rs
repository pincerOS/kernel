#![no_std]

#[cfg(feature = "runtime")]
pub mod runtime;

pub mod spinlock;
pub mod stdout;
pub mod sys;

#[cfg(feature = "thread")]
pub mod thread;

#[cfg(feature = "heap-impl")]
mod heap_impl;

#[macro_use]
#[doc(hidden)]
pub mod macros {
    // https://users.rust-lang.org/t/can-i-conveniently-compile-bytes-into-a-rust-program-with-a-specific-alignment/24049/2
    #[repr(C)]
    pub struct AlignedAs<Align, Bytes: ?Sized> {
        pub _align: [Align; 0],
        pub bytes: Bytes,
    }
    #[macro_export]
    macro_rules! __include_bytes_align {
        ($align_ty:ty, $path:literal) => {{
            // const block expression to encapsulate the static
            use $crate::macros::AlignedAs;
            // this assignment is made possible by CoerceUnsized
            static ALIGNED: &AlignedAs<$align_ty, [u8]> = &AlignedAs {
                _align: [],
                bytes: *include_bytes!($path),
            };
            &ALIGNED.bytes
        }};
    }
}

pub use crate::__include_bytes_align as include_bytes_align;
