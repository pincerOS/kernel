use linked_list_allocator::LockedHeap;

// TODO: use mmap instead
static mut ARENA: [u8; 1 << 23] = [0; 1 << 23];

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub(crate) unsafe fn init_heap() {
    let heap_start = &raw mut ARENA;
    let heap_end = (&raw mut ARENA).wrapping_add(1);
    let heap_size = unsafe { heap_end.byte_offset_from(heap_start) };
    unsafe { ALLOCATOR.lock().init(heap_start.cast(), heap_size as usize) };
}
