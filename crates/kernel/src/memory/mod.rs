mod machine;
mod vmm;

use machine::{at_s1e1r, LeafDescriptor};
pub use vmm::{map_device, map_physical};

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
