use super::machine::{TableDescriptor, TranslationDescriptor};
use super::palloc::{BasePageSize, PhysicalPage, PAGE_ALLOCATOR};
use super::{physical_addr, UnifiedTranslationTable};

#[derive(Copy, Clone)]
pub struct PageTablePtr {
    paddr: usize,
    vaddr: *mut TranslationDescriptor,
    entries: usize,
}

impl PageTablePtr {
    pub fn from_full_page<S: BasePageSize>(page: PhysicalPage<S>) -> Self {
        let paddr = page.paddr;
        let page = PAGE_ALLOCATOR.get().get_mapped_frame(page);
        Self {
            paddr,
            vaddr: page.cast(),
            entries: S::SIZE / size_of::<TranslationDescriptor>(),
        }
    }
    pub fn from_partial_page<S: BasePageSize>(page: PhysicalPage<S>, entries: usize) -> Self {
        let mut this = Self::from_full_page(page);
        assert!(entries <= this.entries);
        this.entries = entries;
        this
    }
    pub fn from_ptr(table: *mut UnifiedTranslationTable) -> Self {
        let paddr = physical_addr(table.addr()).unwrap() as usize;
        Self {
            paddr,
            vaddr: table.cast(),
            entries: size_of::<UnifiedTranslationTable>() / size_of::<TranslationDescriptor>(),
        }
    }
    pub unsafe fn get_entry(&self, idx: usize) -> TranslationDescriptor {
        assert!(idx < self.entries);
        unsafe { core::ptr::read_volatile(self.vaddr.add(idx)) }
    }
    pub unsafe fn set_entry(&mut self, idx: usize, desc: TranslationDescriptor) {
        assert!(idx < self.entries);
        unsafe { core::ptr::write_volatile(self.vaddr.add(idx), desc) }
    }
    pub fn to_descriptor(self) -> TableDescriptor {
        TableDescriptor::new(self.paddr)
    }
    pub fn paddr(self) -> usize {
        self.paddr
    }
    pub fn vaddr(self) -> *mut TranslationDescriptor {
        self.vaddr
    }
}
