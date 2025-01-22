use alloc::alloc::{handle_alloc_error, GlobalAlloc, Layout};
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

// TODO: implement a proper allocator
// TODO: clarify safety requirements of heap initialization

#[global_allocator]
pub static ALLOCATOR: BumpAllocator = unsafe { BumpAllocator::new_uninit() };

pub struct BumpAllocator {
    base: AtomicPtr<()>,
    offset: AtomicUsize,
    max: AtomicUsize,
}
unsafe impl Sync for BumpAllocator {}

impl BumpAllocator {
    pub const unsafe fn new_uninit() -> Self {
        BumpAllocator {
            base: AtomicPtr::new(core::ptr::null_mut()),
            offset: AtomicUsize::new(0),
            max: AtomicUsize::new(0),
        }
    }
    // TODO: safety requirements of initialization?
    pub unsafe fn init(&self, base: *mut (), max: usize) {
        self.base.store(base, Ordering::SeqCst);
        self.max.store(max, Ordering::SeqCst);
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        let base = self.base.load(Ordering::Relaxed).cast::<u8>();
        assert!(!base.is_null());

        let mut cur = self.offset.load(Ordering::Relaxed);

        let start = loop {
            let res = self.offset.compare_exchange(
                cur,
                cur.next_multiple_of(align) + size,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
            match res {
                Ok(start) => break start,
                Err(new) => cur = new,
            }
        };

        let max = self.max.load(Ordering::SeqCst);
        if start.next_multiple_of(align) + size >= max {
            handle_alloc_error(layout);
        }

        let alloc_offset = start.next_multiple_of(align);
        unsafe { base.byte_add(alloc_offset) }
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
