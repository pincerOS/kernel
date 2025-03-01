#![no_std]
#![allow(non_camel_case_types)]

macro_rules! define_endian_int {
    ($name:ident, $base:ty, $from:ident, $to:ident) => {
        #[derive(Copy, Clone, PartialEq, Eq)]
        #[repr(transparent)]
        pub struct $name {
            inner: $base,
        }

        #[must_use]
        #[inline]
        pub const fn $name(val: $base) -> $name {
            $name {
                inner: <$base>::$to(val),
            }
        }

        impl $name {
            #[must_use]
            #[inline]
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
        impl core::convert::From<$name> for $base {
            #[inline]
            fn from(b: $name) -> $base {
                b.get()
            }
        }
        impl core::convert::From<$base> for $name {
            #[inline]
            fn from(b: $base) -> Self {
                $name(b)
            }
        }
    };
}

define_endian_int!(u8_be, u8, from_be, to_be);
define_endian_int!(u16_be, u16, from_be, to_be);
define_endian_int!(u32_be, u32, from_be, to_be);
define_endian_int!(u64_be, u64, from_be, to_be);
define_endian_int!(u128_be, u128, from_be, to_be);

define_endian_int!(i8_be, i8, from_be, to_be);
define_endian_int!(i16_be, i16, from_be, to_be);
define_endian_int!(i32_be, i32, from_be, to_be);
define_endian_int!(i64_be, i64, from_be, to_be);
define_endian_int!(i128_be, i128, from_be, to_be);

define_endian_int!(u8_le, u8, from_le, to_le);
define_endian_int!(u16_le, u16, from_le, to_le);
define_endian_int!(u32_le, u32, from_le, to_le);
define_endian_int!(u64_le, u64, from_le, to_le);
define_endian_int!(u128_le, u128, from_le, to_le);

define_endian_int!(i8_le, i8, from_le, to_le);
define_endian_int!(i16_le, i16, from_le, to_le);
define_endian_int!(i32_le, i32, from_le, to_le);
define_endian_int!(i64_le, i64, from_le, to_le);
define_endian_int!(i128_le, i128, from_le, to_le);
