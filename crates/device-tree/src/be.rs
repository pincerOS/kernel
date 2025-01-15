#![allow(non_camel_case_types)]

macro_rules! define_endian_int {
    ($name:ident, $base:ty, $from:ident, $to:ident) => {
        #[derive(Copy, Clone, PartialEq, Eq)]
        #[repr(transparent)]
        pub struct $name {
            inner: $base,
        }

        #[must_use]
        pub const fn $name(val: $base) -> $name {
            $name {
                inner: <$base>::$to(val),
            }
        }

        impl $name {
            #[must_use]
            pub fn get(self) -> $base {
                <$base>::$from(self.inner)
            }
        }

        unsafe impl bytemuck::Pod for $name {}
        unsafe impl bytemuck::Zeroable for $name {}

        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                core::fmt::Debug::fmt(&self.get(), f)
            }
        }
    };
}

define_endian_int!(u32_be, u32, from_be, to_be);
define_endian_int!(u64_be, u64, from_be, to_be);
