use core::fmt::Formatter;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::sync::atomic::Ordering;

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::sync::{InterruptSpinLock, UnsafeInit};

use super::vmm::{
    __rpi_phys_binary_end_addr, __rpi_phys_binary_start_addr, __rpi_virt_base, DIRECT_MAP_BASE,
    VMEM_INIT_DONE,
};

pub struct PhysicalPage<Size> {
    pub paddr: usize,
    _marker: PhantomData<Size>,
}

impl<S> Clone for PhysicalPage<S> {
    fn clone(&self) -> Self {
        Self {
            paddr: self.paddr.clone(),
            _marker: PhantomData,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct PAddr(pub usize);

pub struct PageAllocator {
    allocator: InterruptSpinLock<BuddyAllocator>,
}

impl<S> PhysicalPage<S> {
    pub fn new(paddr: PAddr) -> Self {
        Self {
            paddr: paddr.0,
            _marker: PhantomData,
        }
    }
}

impl PageAllocator {
    fn init(start: usize, end: usize) -> Self {
        let size = end - start;
        let maximal_size = size.next_power_of_two();
        let floored_base = (start / maximal_size) * maximal_size;
        let allocator = BuddyAllocator::new(floored_base, start, end, 4096);
        PageAllocator {
            allocator: InterruptSpinLock::new(allocator),
        }
    }

    pub fn mark_region_unusable(&self, start: usize, size: usize) {
        self.allocator
            .lock()
            .mark_region_unusable(start, start + size);
    }

    pub fn alloc_range(&self, size: usize, align: usize) -> (PAddr, PAddr) {
        let paddr = self
            .allocator
            .lock()
            .alloc(size, align)
            .expect("Out of physical memory");
        (PAddr(paddr), PAddr(paddr + size))
    }

    pub fn alloc_frame<S: PageClass>(&self) -> PhysicalPage<S> {
        let paddr = self
            .allocator
            .lock()
            .alloc(S::SIZE, S::SIZE)
            .expect("Out of physical memory");
        PhysicalPage {
            paddr,
            _marker: PhantomData,
        }
    }

    pub fn alloc_mapped_frame<S: BasePageSize>(&self) -> PhysicalPage<S> {
        if VMEM_INIT_DONE.load(Ordering::Relaxed) {
            self.alloc_frame()
        } else {
            let mut frame: Box<MaybeUninit<S::Page>> = Box::new_uninit();
            unsafe { core::ptr::write_bytes(frame.as_mut_ptr(), 0, 1) };
            let pointer = Box::into_raw(frame);
            let paddr = pointer as usize - (&raw const __rpi_virt_base) as usize;
            PhysicalPage {
                paddr,
                _marker: PhantomData,
            }
        }
    }

    pub fn get_mapped_frame<S: BasePageSize>(&self, frame: PhysicalPage<S>) -> *mut S::Page {
        if VMEM_INIT_DONE.load(Ordering::Relaxed) {
            unsafe { (DIRECT_MAP_BASE as *mut ()).byte_add(frame.paddr).cast() }
        } else {
            let phys_heap_start = (&raw const __rpi_phys_binary_start_addr) as usize;
            let phys_heap_end = 0x20_0000 * 14;
            assert!(
                frame.paddr >= phys_heap_start && frame.paddr < phys_heap_end,
                "invalid mapped frame: {:?}, phys_heap_start {:#x}, phys_heap_end {:#x}",
                frame,
                phys_heap_start,
                phys_heap_end,
            );
            unsafe { (&raw mut __rpi_virt_base).byte_add(frame.paddr).cast() }
        }
    }

    pub fn dealloc_frame<S: PageClass>(&self, frame: PhysicalPage<S>) {
        let phys_heap_start = (&raw const __rpi_phys_binary_end_addr) as usize;
        let phys_heap_end = 0x20_0000 * 14;
        if frame.paddr >= phys_heap_start && frame.paddr < phys_heap_end {
            // From the kernel heap...
            // TODO: use direct mapped physmem instead (or specialization, but that will never be stable)
            let vaddr = unsafe { (&raw mut __rpi_virt_base).byte_add(frame.paddr) };
            const { assert!(S::SIZE == 4096 || S::SIZE == 16384 || S::SIZE == 65536) };
            if S::SIZE == 4096 {
                let allocation = unsafe { Box::<MaybeUninit<Page4k>>::from_raw(vaddr.cast()) };
                drop(allocation);
            } else if S::SIZE == 16384 {
                let allocation = unsafe { Box::<MaybeUninit<Page16k>>::from_raw(vaddr.cast()) };
                drop(allocation);
            } else if S::SIZE == 65536 {
                let allocation = unsafe { Box::<MaybeUninit<Page64k>>::from_raw(vaddr.cast()) };
                drop(allocation);
            } else {
                panic!();
            }
        } else {
            self.allocator.lock().free(frame.paddr, S::SIZE, S::SIZE);
        }
    }
}

pub static PAGE_ALLOCATOR: UnsafeInit<PageAllocator> = unsafe { UnsafeInit::uninit() };

pub unsafe fn init_physical_alloc(paddr_start: usize, paddr_end: usize) {
    let allocator = PageAllocator::init(paddr_start, paddr_end);
    unsafe { PAGE_ALLOCATOR.init(allocator) };
}

trait Sealed {}

#[allow(private_bounds)]
pub trait PageClass: Sealed {
    const SIZE: usize;
    const BITS: usize = Self::SIZE.ilog2() as usize;
    const NAME: &str;
}

pub trait BasePageSize: PageClass {
    type Page;
}

pub struct Size4KiB;

pub struct Size16KiB;

pub struct Size64KiB;

pub struct Size2MiB;

pub struct Size1GiB;

#[repr(C, align(4096))]
pub struct Page4k([u8; 4096]);

#[repr(C, align(16384))]
pub struct Page16k([u8; 16384]);

#[repr(C, align(16384))]
pub struct Page64k([u8; 65536]);

impl Sealed for Size4KiB {}
impl PageClass for Size4KiB {
    const SIZE: usize = 4096;
    const NAME: &str = "Size4KiB";
}
impl BasePageSize for Size4KiB {
    type Page = Page4k;
}

impl Sealed for Size16KiB {}
impl PageClass for Size16KiB {
    const SIZE: usize = 16384;
    const NAME: &str = "Size4KiB";
}
impl BasePageSize for Size16KiB {
    type Page = Page16k;
}

impl Sealed for Size64KiB {}
impl PageClass for Size64KiB {
    const SIZE: usize = 64384;
    const NAME: &str = "Size4KiB";
}
impl BasePageSize for Size64KiB {
    type Page = Page64k;
}

impl Sealed for Size2MiB {}
impl PageClass for Size2MiB {
    const SIZE: usize = 4096 * 512;
    const NAME: &str = "Size2MiB";
}

impl Sealed for Size1GiB {}
impl PageClass for Size1GiB {
    const SIZE: usize = 4096 * 512 * 512;
    const NAME: &str = "Size1GiB";
}

impl<Size: PageClass> core::fmt::Debug for PhysicalPage<Size> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "PhysicalPage<{}>({:#x})", Size::NAME, self.paddr)
    }
}

pub struct Bitset(Box<[usize]>);

impl Bitset {
    const BITS_PER_WORD: usize = size_of::<usize>() * 8;

    pub fn new(bits: usize) -> Self {
        let mut slice = Box::new_uninit_slice(bits.div_ceil(Self::BITS_PER_WORD));
        for word in &mut slice {
            word.write(0);
        }
        Self(unsafe { slice.assume_init() })
    }

    pub fn set_bit(&mut self, idx: usize) {
        let word = idx / Self::BITS_PER_WORD;
        let bit = idx % Self::BITS_PER_WORD;
        self.0[word] |= 1 << bit;
    }
    pub fn toggle_bit(&mut self, idx: usize) {
        let word = idx / Self::BITS_PER_WORD;
        let bit = idx % Self::BITS_PER_WORD;
        self.0[word] ^= 1 << bit;
    }
    pub fn clear_bit(&mut self, idx: usize) {
        let word = idx / Self::BITS_PER_WORD;
        let bit = idx % Self::BITS_PER_WORD;
        self.0[word] &= !(1 << bit);
    }
    pub fn get(&self, idx: usize) -> bool {
        let word = idx / Self::BITS_PER_WORD;
        let bit = idx % Self::BITS_PER_WORD;
        self.0[word] & (1 << bit) != 0
    }
}

// A buddy allocator based on a design by Niklas Gray:
// https://bitsquid.blogspot.com/2015/08/allocation-adventures-3-buddy-allocator.html
pub struct BuddyAllocator {
    bitset: Bitset,
    base: usize,
    _size: usize,
    size_log2: usize,
    levels: usize,
    min_size: usize,
    freelist: Freelist,
}

struct Freelist {
    freelists: Box<[Vec<usize>]>,
}

impl Freelist {
    fn insert(&mut self, level: usize, block: usize) {
        // TODO: How much does the ordering of the nodes matter?
        // TODO: intrusive list?
        let list = &mut self.freelists[level];
        // TODO: This will fail under load; need a secondary heap
        // or an intrusive freelist to avoid issues
        assert!(list.len() < list.capacity());
        debug_assert!(!list.contains(&block));
        list.push(block);
    }

    fn pop_smallest(&mut self, max_level: usize) -> Option<(usize, usize)> {
        for (level, freelist) in self.freelists[0..=max_level].iter_mut().enumerate().rev() {
            if let Some(block) = freelist.pop() {
                return Some((level, block));
            }
        }
        None
    }

    fn remove(&mut self, cur_level: usize, sibling: usize) {
        // TODO: efficiency
        let cur_freelist = &mut self.freelists[cur_level as usize];
        let sibling_idx = cur_freelist.iter().position(|i| *i == sibling).unwrap();
        cur_freelist.remove(sibling_idx);
    }

    fn print_freelists(&self) {
        for freelist in self.freelists.iter().enumerate() {
            println!("{:?}", freelist);
        }
    }

    fn retain(&mut self, level: usize, mut handler: impl FnMut(usize) -> bool) {
        self.freelists[level].retain(|i| handler(*i));
    }
}

impl BuddyAllocator {
    pub fn new(base: usize, start: usize, end: usize, min_size: usize) -> Self {
        assert!(min_size.is_power_of_two());
        let size = end - base;
        let bits = size.next_power_of_two().ilog2();
        let levels = bits.saturating_sub(min_size.ilog2()) + 1;

        // Bitset storing (L_FREE xor R_FREE)
        let bitset = Bitset::new(1 << (levels - 1));
        let mut freelists = Vec::with_capacity(levels as usize);
        for level in 0..levels {
            let count = (1 << level).min(8192);
            freelists.push(Vec::with_capacity(count));
        }
        let freelists = freelists.into_boxed_slice();

        let mut this = BuddyAllocator {
            bitset,
            base,
            _size: size,
            size_log2: bits as usize,
            levels: levels as usize,
            min_size,
            freelist: Freelist { freelists },
        };

        if start == base && size == (1 << bits) {
            this.freelist.insert(0, 0);
        } else {
            let cutoff_start = start - base;
            let cutoff_end = end - base;

            this.freelist.insert(0, 0);
            this.mark_region_unusable(0, cutoff_start);
            this.mark_region_unusable(cutoff_end, 1 << bits);
        }
        this
    }

    pub fn mark_region_unusable(&mut self, range_start: usize, range_end: usize) {
        // assumes no allocations have been done yet within this range
        // iterate through all free blocks that intersect with this range
        // four* cases:
        // 0. the block is not in the range
        // 1. the block is fully contained within the range
        // 2. the range is fully contained within the block
        // 3. the block contains the end of the range
        // 4. the block contains the start of the range

        // Remove from the free list; then
        // Case 0:
        // - Re-add to the free list
        // Case 1:
        // - Mark the block as used
        // Case 2:
        // - Mark as split
        // - Recurse on left and right children
        // Case 3 & Case 4:
        // - Mark as split
        // - Recurse on left and right children
        // Fanout is bounded:
        // - in case 2:
        //    - one child will be case 0 and the other case 1 or 2
        //    - or, one child will be case 3 and the other 4
        // - in cases 3/4:
        //    - one child will be case 0/1, and the other in 3/4

        // At each layer,
        // - Any number of cases 0 and 1
        // - Either
        //    - one case 2
        //    - up to one of each case 3 and case 4

        let get_start_end = |level: usize, i: usize| -> (usize, usize) {
            let block_size = 1 << (self.size_log2 - level);
            let level_idx_start = (1 << level) - 1;
            let block_start = (i - level_idx_start) * block_size;
            let block_end = block_start + block_size;
            (block_start, block_end)
        };
        let case_0 = |start, end| end <= range_start || start >= range_end;
        let case_1 = |start, end| range_start <= start && end <= range_end;

        let mut queue = tinyvec::ArrayVec::<[_; 4]>::new();

        for level in 0..self.levels {
            let prev_queue = core::mem::take(&mut queue);

            if level < self.levels - 1 {
                let mut handler = |i| {
                    let (block_start, block_end) = get_start_end(level, i);
                    if case_0(block_start, block_end) {
                        true // Case 0, don't remove from the free list
                    } else if case_1(block_start, block_end) {
                        // Case 1
                        Self::toggle_at(&mut self.bitset, i);
                        false
                    } else {
                        // Case 2, 3, or 4
                        Self::toggle_at(&mut self.bitset, i);
                        let left = Self::child_idx_base(i);
                        let right = left + 1;
                        queue.push(left);
                        queue.push(right);
                        false
                    }
                };

                self.freelist.retain(level, &mut handler);

                for elem in prev_queue {
                    if handler(elem) {
                        self.freelist.insert(level, elem);
                    }
                }
            } else {
                // Last level; treat all cases but case 0 as case 1
                let mut handler = |i| {
                    let (block_start, block_end) = get_start_end(level, i);
                    if case_0(block_start, block_end) {
                        true // Case 0, don't remove from the free list
                    } else {
                        // Case 1, 2, 3, 4
                        Self::toggle_at(&mut self.bitset, i);
                        false
                    }
                };

                self.freelist.retain(level, &mut handler);

                for elem in prev_queue {
                    if handler(elem) {
                        self.freelist.insert(level, elem);
                    }
                }
            }
        }
    }

    fn toggle_at(bitset: &mut Bitset, i: usize) {
        if i != 0 {
            bitset.toggle_bit(Self::bit_idx(i));
        }
    }

    fn level_for_size(&self, size: usize) -> Option<usize> {
        let alloc_size = size.next_power_of_two().clamp(self.min_size, usize::MAX);
        self.size_log2.checked_sub(alloc_size.ilog2() as usize)
    }

    fn addr_idx(&self, addr: usize, level: usize) -> usize {
        let offset = addr - self.base;
        (1 << level) + (offset >> (self.size_log2 - level)) - 1
    }
    fn idx_addr(&self, idx: usize, level: usize) -> usize {
        let level_idx_start = (1 << level) - 1;
        let offset = (idx - level_idx_start) << (self.size_log2 - level);
        self.base + offset
    }
    fn parent_idx(idx: usize) -> usize {
        (idx - 1) >> 1
    }
    fn child_idx_base(idx: usize) -> usize {
        (idx << 1) + 1
    }
    fn sibling_idx(idx: usize) -> usize {
        ((idx - 1) ^ 1) + 1
    }

    #[track_caller]
    fn bit_idx(idx: usize) -> usize {
        (idx - 1) >> 1
    }

    fn alloc_at_level(&mut self, level: usize) -> Option<usize> {
        assert!(level < self.levels);
        let (mut block_level, mut block_idx) = self.freelist.pop_smallest(level)?;

        // Split block repeatedly, if needed
        while block_level < level {
            Self::toggle_at(&mut self.bitset, block_idx); // Mark as split

            let left_child = Self::child_idx_base(block_idx);
            let right_child = left_child + 1;

            self.freelist.insert(block_level + 1, right_child);

            block_idx = left_child;
            block_level += 1;
        }

        Self::toggle_at(&mut self.bitset, block_idx); // Mark as allocated

        Some(block_idx)
    }

    fn free_block(&mut self, mut idx: usize) {
        assert!(idx < (1 << self.levels));

        Self::toggle_at(&mut self.bitset, idx); // Mark as free

        let mut cur_level = (idx + 1).ilog2();

        while idx != 0 {
            let parent = Self::parent_idx(idx);
            let sibling = Self::sibling_idx(idx);
            if self.bitset.get(Self::bit_idx(idx)) {
                // (cur ^ sibling) is 1, so sibling is still allocated
                // Can't coalesce further
                break;
            }

            // Both children are free, so mark the parent as free
            Self::toggle_at(&mut self.bitset, parent);

            // Remove the sibling from the freelist (coalesce it into the parent)
            self.freelist.remove(cur_level as usize, sibling);

            idx = parent;
            cur_level -= 1;
        }

        // Add the coalesced block to the free list
        self.freelist.insert(cur_level as usize, idx);
    }

    pub fn alloc(&mut self, size: usize, align: usize) -> Option<usize> {
        let size = size.clamp(align, usize::MAX);
        let level = self.level_for_size(size)?;
        let idx = self.alloc_at_level(level)?;
        let ptr = self.idx_addr(idx, level);
        // println!("alloc({size}) -> {ptr:#x}");
        Some(ptr)
    }

    pub fn free(&mut self, ptr: usize, size: usize, align: usize) {
        let size = size.clamp(align, usize::MAX);
        // println!("free({ptr:#x}, {size})");
        let level = self.level_for_size(size).unwrap();
        let idx = self.addr_idx(ptr, level);
        self.free_block(idx);
    }

    pub fn print_state(&self) {
        let width = 64;
        for level in 0..self.levels - 1 {
            let pad = (width >> level) - 2;
            for idx in (1 << level) - 1..(1 << (level + 1)) - 1 {
                let mode = match self.bitset.get(idx) {
                    false => '=',
                    true => 'X',
                };
                let component = alloc::format!("{idx} {mode}");
                print!("[{:^pad$}]", component);
            }
            println!();
        }
        {
            let level = self.levels - 1;
            let pad = (width >> level) - 2;
            for idx in (1 << level) - 1..(1 << (level + 1)) - 1 {
                let component = alloc::format!("{idx}");
                print!("[{:^pad$}]", component);
            }
            println!();
        }
        self.freelist.print_freelists();
    }
}
