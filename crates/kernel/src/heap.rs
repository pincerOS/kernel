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
        //self.base.store(base, Ordering::SeqCst);
        self.offset.store(base as usize, Ordering::SeqCst);
        self.max.store(max + (base as usize), Ordering::SeqCst);
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        let base = self.base.load(Ordering::Relaxed).cast::<u8>();
        assert!(self.max.load(Ordering::Relaxed) != 0);
        //assert!(!base.is_null());

        let mut cur = self.offset.load(Ordering::Relaxed);
        let max = self.max.load(Ordering::Relaxed);

        let start = loop {
            let new = (|| {
                let aligned = cur.checked_next_multiple_of(align)?;
                let end = aligned.checked_add(size)?;
                (end < max).then_some(end)
            })();
            let new = new.unwrap_or_else(|| handle_alloc_error(layout));
            let ord = Ordering::Relaxed;
            let res = self.offset.compare_exchange(cur, new, ord, ord);
            match res {
                Ok(start) => break start,
                Err(new) => cur = new,
            }
        };

        let alloc_offset = start.next_multiple_of(align);
        unsafe { base.byte_add(alloc_offset) }
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
