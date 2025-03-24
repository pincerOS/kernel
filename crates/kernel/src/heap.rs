use alloc::alloc::{handle_alloc_error, GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

// TODO: implement a proper allocator
// TODO: clarify safety requirements of heap initialization

use linked_list_allocator::LockedHeap;

use crate::arch::memory::machine::LeafDescriptor;
use crate::arch::memory::palloc::{PhysicalPage, Size4KiB, PAGE_ALLOCATOR};
use crate::arch::memory::table::PageTablePtr;
use crate::arch::memory::vmm::PAGE_SIZE;
use crate::sync::SpinLock;

pub struct HeapStats {
    pub used: usize,
}

pub fn stats() -> HeapStats {
    // ALLOCATOR.offset.load(Ordering::SeqCst)
    HeapStats {
        used: ALLOCATOR.lock().used(),
    }
}

#[global_allocator]
pub static ALLOCATOR: LockedHeap = LockedHeap::empty();

// #[global_allocator]
// pub static ALLOCATOR: BumpAllocator = unsafe { BumpAllocator::new_uninit() };

pub struct BumpAllocator {
    pub offset: AtomicUsize,
    pub max: AtomicUsize,
}
unsafe impl Sync for BumpAllocator {}

impl BumpAllocator {
    pub const unsafe fn new_uninit() -> Self {
        BumpAllocator {
            offset: AtomicUsize::new(0),
            max: AtomicUsize::new(0),
        }
    }
    // TODO: safety requirements of initialization?
    pub unsafe fn init(&self, base: *mut (), size: usize) {
        self.offset.store(base as usize, Ordering::SeqCst);
        self.max.store(base as usize + size, Ordering::SeqCst);
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        assert!(self.max.load(Ordering::Relaxed) != 0);

        let mut cur = self.offset.load(Ordering::Relaxed);
        let max = self.max.load(Ordering::Relaxed);

        let start = loop {
            let new = (|| {
                let aligned = cur.checked_next_multiple_of(align)?;
                let end = aligned.checked_add(size)?;
                (end < max).then_some(end)
            })();
            let new = new.unwrap_or_else(|| {
                println!("Heap full; requested size: {}, align: {}, cur: {:#x}, end: {:#x}", layout.size(), layout.align(), cur, max);
                handle_alloc_error(layout)
            });
            let ord = Ordering::Relaxed;
            let res = self.offset.compare_exchange(cur, new, ord, ord);
            match res {
                Ok(start) => break start,
                Err(new) => cur = new,
            }
        };

        let alloc_offset = start.next_multiple_of(align);
        alloc_offset as *mut u8
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}


pub struct VirtAllocator {
    pub base: usize,
    pub offset: AtomicUsize,
    pub max: AtomicUsize,
}
unsafe impl Sync for VirtAllocator {}

impl VirtAllocator {
    // TODO: safety requirements of initialization?
    pub unsafe fn new(base: *mut (), size: usize) -> Self {
        Self {
            base: base as usize,
            offset: AtomicUsize::new(base as usize),
            max: AtomicUsize::new(base as usize + size),
        }
    }
}

unsafe impl GlobalAlloc for VirtAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().next_multiple_of(PAGE_SIZE);
        let align = layout.align().next_multiple_of(PAGE_SIZE);

        assert!(self.max.load(Ordering::Relaxed) != 0);

        let mut cur = self.offset.load(Ordering::Relaxed);
        let max = self.max.load(Ordering::Relaxed);

        let start = loop {
            let new = (|| {
                let aligned = cur.checked_next_multiple_of(align)?;
                let end = aligned.checked_add(size)?;
                (end < max).then_some(end)
            })();
            let new = new.unwrap_or_else(|| {
                println!("Heap full; requested size: {}, align: {}, cur: {:#x}, end: {:#x}", layout.size(), layout.align(), cur, max);
                handle_alloc_error(layout)
            });
            let ord = Ordering::Relaxed;
            let res = self.offset.compare_exchange(cur, new, ord, ord);
            match res {
                Ok(start) => break start,
                Err(new) => cur = new,
            }
        };

        let vaddr_base = start.next_multiple_of(align);

        let table = &raw mut crate::arch::memory::KERNEL_UNIFIED_TRANSLATION_TABLE;
        let table = PageTablePtr::from_ptr(table);
        let pages = size.div_ceil(PAGE_SIZE);
        for page in 0..pages {
            let pa = PAGE_ALLOCATOR.get().alloc_frame::<Size4KiB>();
            let vaddr = vaddr_base + page * PAGE_SIZE;
            let descriptor = LeafDescriptor::new(pa.paddr);
            unsafe {
                crate::arch::memory::vmm::set_translation_descriptor(table, vaddr, 3, 0, descriptor.into(), true).unwrap()
            };
        }

        vaddr_base as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size().next_multiple_of(PAGE_SIZE);
        let align = layout.align().next_multiple_of(PAGE_SIZE);

        assert!(ptr as usize % PAGE_SIZE == 0);
        assert!(ptr as usize % align == 0);
        let heap_range = self.base .. self.max.load(Ordering::Relaxed);
        assert!(heap_range.contains(&(ptr as usize)), "{ptr:p} not in heap range {heap_range:?}");

        let vaddr_base = ptr as usize;

        let table = &raw mut crate::arch::memory::KERNEL_UNIFIED_TRANSLATION_TABLE;
        let table = PageTablePtr::from_ptr(table);
        let pages = size.div_ceil(PAGE_SIZE);
        for page in 0..pages {
            let vaddr = vaddr_base + page * PAGE_SIZE;
            let desc = unsafe {
                crate::arch::memory::vmm::get_translation_descriptor(table, vaddr, 3, 0).unwrap()
            };
            let leaf = unsafe { desc.leaf };
            assert!(leaf.is_valid() && leaf.contains(LeafDescriptor::IS_PAGE_DESCRIPTOR), "invalid leaf {leaf:?}");

            PAGE_ALLOCATOR.get().dealloc_frame(PhysicalPage::<Size4KiB>::new(leaf.get_pa()));
        }
    }
}
