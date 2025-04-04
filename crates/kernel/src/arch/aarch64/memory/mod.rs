pub mod machine;
pub mod palloc;
pub mod vmm;

use core::arch::asm;

use machine::{at_s1e1r, LeafDescriptor};
pub use vmm::{
    create_user_region, init_physical_alloc, map_device, map_device_block, map_va_to_pa,
    map_physical, map_physical_noncacheable, UnifiedTranslationTable,
    KERNEL_UNIFIED_TRANSLATION_TABLE,
};

pub use machine::at_s1e0r;

pub const INIT_TCR_EL1: u64 = machine::TcrEl1::empty()
    .set_t0sz(39) // 25 bits of address translation
    .difference(machine::TcrEl1::EPD0)
    .set_irgn0(0b01)
    .set_orgn0(0b01)
    .set_sh0(0b10)
    .set_tg0(machine::PageSize::Size4KiB)
    .set_t1sz(39) // 25 bits of address translation
    .difference(machine::TcrEl1::A1)
    .difference(machine::TcrEl1::EPD1)
    .set_irgn1(0b01)
    .set_orgn1(0b01)
    .set_sh1(0b10)
    .set_tg1(machine::PageSize::Size4KiB)
    .set_ips(0b101)
    .union(machine::TcrEl1::AS)
    .difference(machine::TcrEl1::TBI0)
    .difference(machine::TcrEl1::TBI1)
    .bits();

pub const KERNEL48_USER25_TCR_EL1: u64 = machine::TcrEl1::empty()
    .set_t0sz(39) // 25 bits of address translation
    .difference(machine::TcrEl1::EPD0)
    .set_irgn0(0b01)
    .set_orgn0(0b01)
    .set_sh0(0b10)
    .set_tg0(machine::PageSize::Size4KiB)
    .set_t1sz(16) // 48 bits of address translation
    .difference(machine::TcrEl1::A1)
    .difference(machine::TcrEl1::EPD1)
    .set_irgn1(0b01)
    .set_orgn1(0b01)
    .set_sh1(0b10)
    .set_tg1(machine::PageSize::Size4KiB)
    .set_ips(0b101)
    .union(machine::TcrEl1::AS)
    .difference(machine::TcrEl1::TBI0)
    .difference(machine::TcrEl1::TBI1)
    .bits();

pub const KERNEL48_USER48_TCR_EL1: u64 = machine::TcrEl1::empty()
    .set_t0sz(16) // 48 bits of address translation
    .difference(machine::TcrEl1::EPD0)
    .set_irgn0(0b01)
    .set_orgn0(0b01)
    .set_sh0(0b10)
    .set_tg0(machine::PageSize::Size4KiB)
    .set_t1sz(16) // 48 bits of address translation
    .difference(machine::TcrEl1::A1)
    .difference(machine::TcrEl1::EPD1)
    .set_irgn1(0b01)
    .set_orgn1(0b01)
    .set_sh1(0b10)
    .set_tg1(machine::PageSize::Size4KiB)
    .set_ips(0b101)
    .union(machine::TcrEl1::AS)
    .difference(machine::TcrEl1::TBI0)
    .difference(machine::TcrEl1::TBI1)
    .bits();

pub const INIT_TRANSLATION: u64 = LeafDescriptor::new(0)
    .set_global()
    .clear_pxn()
    .difference(LeafDescriptor::IS_PAGE_DESCRIPTOR)
    .bits();

pub fn physical_addr(va: usize) -> Option<u64> {
    at_s1e1r(va)
        .ok()
        .map(|res| res.base_pa() + (va & 0xFFF) as u64)
}

pub unsafe fn init() {
    unsafe {
        vmm::init();
    }
}

// Note: this may not need to be unsafe
pub unsafe fn clean_physical_buffer_for_device(va: *mut (), bytes: usize) {
    let va = va.addr();
    for ptr in va..(va + bytes) {
        // clean each byte
        // TODO: only invoke the cleaning once per cache line by using the cache registers to find line width
        unsafe {
            asm! {
                "dc cvac, {ptr}",
                ptr = in(reg) ptr,
                options(readonly, nostack, preserves_flags)
            }
        }
    }
    // enforce memory barrier between this and subsequent memory operations
    // must be inserted at some point before the device access, and this is a reasonable point
    unsafe {
        asm! {
            "dmb sy",
            options(readonly, nostack, preserves_flags)
        }
    }
}
pub unsafe fn invalidate_physical_buffer_for_device(va: *mut (), bytes: usize) {
    // enforce memory barrier between this and prior memory operations
    // probably needs to be inserted (?) at some point after the device work completes, and this is a reasonable point
    unsafe {
        asm! {
            "dmb sy",
            options(readonly, nostack, preserves_flags)
        }
    }
    let va = va.addr();
    for ptr in va..(va + bytes) {
        // invalidate each byte
        // TODO: only invoke the invalidating once per cache line by using the cache registers to find line width
        unsafe {
            asm! {
                "dc ivac, {ptr}",
                ptr = in(reg) ptr,
                options(readonly, nostack, preserves_flags)
            }
        }
    }
}
