pub mod machine;
pub mod palloc;
pub mod vmm;

use core::arch::asm;

use machine::{at_s1e1r, LeafDescriptor};
pub use vmm::{map_device, map_device_block, map_physical, map_physical_noncacheable};

pub use machine::at_s1e0r;
pub use vmm::{create_user_region, init_physical_alloc, map_pa_to_va};

pub const INIT_TCR_EL1: u64 = machine::TcrEl1::default().bits();
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
