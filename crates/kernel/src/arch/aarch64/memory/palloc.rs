use core::fmt::Formatter;
use core::marker::PhantomData;
use core::mem::MaybeUninit;

use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;

use crate::sync::{SpinLock, UnsafeInit};

pub struct PhysicalPage<Size> {
    pub paddr: usize,
    _marker: PhantomData<Size>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct PAddr(pub usize);

pub struct PageAllocator {
    allocator: SpinLock<BuddyAllocator>,
}

#[repr(C, align(4096))]
pub struct Page([u8; 4096]);

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
            allocator: SpinLock::new(allocator),
        }
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

    pub fn alloc_mapped_frame(&self) -> PhysicalPage<Size4KiB> {
        let mut frame: Box<MaybeUninit<Page>> = Box::new_uninit();
        unsafe { core::ptr::write_bytes(frame.as_mut_ptr(), 0, 1) };
        let pointer = Box::into_raw(frame);
        let paddr = pointer as usize - (&raw const super::vmm::__rpi_virt_base) as usize;

        PhysicalPage {
            paddr,
            _marker: PhantomData,
        }
    }

    #[track_caller]
    pub fn get_mapped_frame(&self, frame: PhysicalPage<Size4KiB>) -> *mut Page {
        let phys_heap_start = (&raw const super::vmm::__rpi_phys_binary_end_addr) as usize;
        let phys_heap_end = 0x20_0000 * 14;
        assert!(
            frame.paddr >= phys_heap_start && frame.paddr < phys_heap_end,
            "frame {:?}, phys_heap_start {:#x}, phys_heap_end {:#x}",
            frame,
            phys_heap_start,
            phys_heap_end,
        );

        unsafe {
            (&raw mut super::vmm::__rpi_virt_base)
                .byte_add(frame.paddr)
                .cast()
        }
    }

    pub fn dealloc_frame<S: PageClass>(&self, frame: PhysicalPage<S>) {
        let phys_heap_start = (&raw const super::vmm::__rpi_phys_binary_end_addr) as usize;
        let phys_heap_end = 0x20_0000 * 14;
        if frame.paddr >= phys_heap_start && frame.paddr < phys_heap_end {
            // From the kernel heap...
            let allocation;
            unsafe {
                let vaddr = (&raw mut super::vmm::__rpi_virt_base).byte_add(frame.paddr);
                allocation = Box::<MaybeUninit<Page>>::from_raw(vaddr.cast());
            }
            drop(allocation);
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
    const BITS: usize;
    const NAME: &str;
}

pub struct Size4KiB;
pub struct Size2MiB;
pub struct Size1GiB;

impl Sealed for Size4KiB {}
impl PageClass for Size4KiB {
    const SIZE: usize = 4096;
    const BITS: usize = 12;
    const NAME: &str = "Size4KiB";
}

impl Sealed for Size2MiB {}
impl PageClass for Size2MiB {
    const SIZE: usize = 4096 * 512;
    const BITS: usize = 21;
    const NAME: &str = "Size2MiB";
}

impl Sealed for Size1GiB {}
impl PageClass for Size1GiB {
    const SIZE: usize = 4096 * 512 * 512;
    const BITS: usize = 30;
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

    // TODO: ???? (intrusive version?)
    freelists: Box<[VecDeque<usize>]>,
}

impl BuddyAllocator {
    pub fn new(base: usize, start: usize, end: usize, min_size: usize) -> Self {
        assert!(min_size.is_power_of_two());
        let size = end - base;
        let bits = size.next_power_of_two().ilog2();
        let levels = bits.saturating_sub(min_size.ilog2()) + 1;

        // Bitset storing (L_FREE xor R_FREE)
        let bitset = Bitset::new(1 << (levels - 1));
        let freelists = alloc::vec![VecDeque::new(); levels as usize].into_boxed_slice();

        let mut this = BuddyAllocator {
            bitset,
            base,
            _size: size,
            size_log2: bits as usize,
            levels: levels as usize,
            min_size,
            freelists,
        };

        if start == base && size == (1 << bits) {
            this.freelists[0].push_front(0);
        } else {
            let cutoff_start = start - base;
            let cutoff_end = end - base;

            this.freelists[0].push_front(0);
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

        for level in 0..self.levels {
            let block_size = 1 << (self.size_log2 - level);
            let level_idx_start = (1 << level) - 1;

            let mut iter = self.freelists[level..].iter_mut();
            let cur_list = iter.next().unwrap();
            if let Some(next_list) = iter.next() {
                cur_list.retain(|&i| {
                    let block_start = (i - level_idx_start) * block_size;
                    let block_end = block_start + block_size;
                    if block_end <= range_start || block_start >= range_end {
                        true // Case 0, don't remove from the free list
                    } else if range_start <= block_start && block_end <= range_end {
                        // Case 1
                        if i != 0 {
                            self.bitset.toggle_bit(Self::bit_idx(i));
                        }
                        false
                    } else {
                        // Case 2, 3, or 4
                        if i != 0 {
                            self.bitset.toggle_bit(Self::bit_idx(i));
                        }
                        let left = Self::child_idx_base(i);
                        let right = left + 1;
                        next_list.push_back(left);
                        next_list.push_back(right);
                        false
                    }
                });
            } else {
                // Last level; treat all cases but case 0 as case 1
                cur_list.retain(|&i| {
                    let block_start = (i - level_idx_start) * block_size;
                    let block_end = block_start + block_size;
                    if block_end <= range_start || block_start >= range_end {
                        true // Case 0, don't remove from the free list
                    } else {
                        // Case 1, 2, 3, 4
                        if i != 0 {
                            self.bitset.toggle_bit(Self::bit_idx(i));
                        }
                        false
                    }
                });
            }
        }
    }

    fn level_for_size(&self, size: usize) -> Option<usize> {
        let alloc_size = size.next_power_of_two().min(self.min_size);
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
        let mut free_block = None;
        for (level, freelist) in self.freelists[0..=level].iter_mut().enumerate().rev() {
            if let Some(block) = freelist.pop_front() {
                free_block = Some((level, block));
                break;
            }
        }

        let (mut block_level, mut block_idx) = free_block?;

        // Split block repeatedly, if needed
        while block_level < level {
            if block_idx != 0 {
                self.bitset.toggle_bit(Self::bit_idx(block_idx)); // Mark as split
            }
            let left_child = Self::child_idx_base(block_idx);
            let right_child = left_child + 1;

            self.freelists[block_level + 1].push_front(right_child);

            block_idx = left_child;
            block_level += 1;
        }

        if block_idx != 0 {
            self.bitset.toggle_bit(Self::bit_idx(block_idx)); // Mark as allocated
        }
        Some(block_idx)
    }

    fn free_block(&mut self, mut idx: usize) {
        assert!(idx < (1 << self.levels));

        if idx != 0 {
            self.bitset.toggle_bit(Self::bit_idx(idx)); // Mark as free
        }

        let mut cur_level = (idx + 1).ilog2();

        while idx != 0 {
            let parent = Self::parent_idx(idx);
            let sibling = Self::sibling_idx(idx);
            if self.bitset.get(Self::bit_idx(idx)) {
                // (cur ^ sibling) is 1, so sibling is still allocated
                // Can't coalesce further
                break;
            }
            if parent != 0 {
                // Both children are free, so mark the parent as free
                self.bitset.toggle_bit(Self::bit_idx(parent));
            }

            // Remove the sibling from the freelist (coalesce it into the parent)
            // TODO: efficiency
            let cur_freelist = &mut self.freelists[cur_level as usize];
            let sibling_idx = cur_freelist.iter().position(|i| *i == sibling).unwrap();
            cur_freelist.remove(sibling_idx);

            idx = parent;
            cur_level -= 1;
        }

        // TODO: How much does the ordering of the nodes matter?
        // Add the coalesced block to the free list
        self.freelists[cur_level as usize].push_front(idx);
    }

    pub fn alloc(&mut self, size: usize, align: usize) -> Option<usize> {
        let size = size.min(align); // TODO: ensure base is aligned
        let level = self.level_for_size(size)?;
        let idx = self.alloc_at_level(level)?;
        let ptr = self.idx_addr(idx, level);
        Some(ptr)
    }

    pub fn free(&mut self, ptr: usize, size: usize, align: usize) {
        let size = size.min(align);
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
        for freelist in self.freelists.iter().enumerate() {
            println!("{:?}", freelist);
        }
    }
}
