#![allow(non_camel_case_types)]

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct u32_be {
    inner: u32,
}
#[must_use]
pub const fn u32_be(val: u32) -> u32_be {
    u32_be {
        inner: u32::to_be(val),
    }
}
impl u32_be {
    #[must_use]
    pub fn get(self) -> u32 {
        u32::from_be(self.inner)
    }
}
unsafe impl bytemuck::Pod for u32_be {}
unsafe impl bytemuck::Zeroable for u32_be {}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct u64_be {
    inner: u64,
}
#[must_use]
pub const fn u64_be(val: u64) -> u64_be {
    u64_be {
        inner: u64::to_be(val),
    }
}
impl u64_be {
    #[must_use]
    pub fn get(self) -> u64 {
        u64::from_be(self.inner)
    }
}
unsafe impl bytemuck::Pod for u64_be {}
unsafe impl bytemuck::Zeroable for u64_be {}

impl core::fmt::Debug for u32_be {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.get(), f)
    }
}
impl core::fmt::Debug for u64_be {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.get(), f)
    }
}
