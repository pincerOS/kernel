use core::fmt::Formatter;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::boxed::Box;

use crate::sync::UnsafeInit;

pub struct PhysicalPage<Size> {
    pub paddr: usize,
    _marker: PhantomData<Size>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct PAddr(pub usize);

pub struct PageAllocator {
    _base: usize,
    current: AtomicUsize,
    max: usize,
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
        let base = start;
        let max = end - base;
        PageAllocator {
            _base: base,
            current: AtomicUsize::new(base),
            max,
        }
    }

    pub fn alloc_range(&self, size: usize, align: usize) -> (PAddr, PAddr) {
        let align = align.max(4096);
        let mut cur = self.current.load(Ordering::Relaxed);

        let start = loop {
            let next = cur.next_multiple_of(align) + size; // TODO: prevent wrapping
            let ord = Ordering::Relaxed;
            let res = self.current.compare_exchange(cur, next, ord, ord);
            match res {
                Ok(start) => break start,
                Err(new) => cur = new,
            }
        };

        if start.next_multiple_of(align) + size > self.max {
            panic!("Out of physical memory")
        }

        let paddr = start.next_multiple_of(align);
        (PAddr(paddr), PAddr(paddr + size))
    }

    pub fn alloc_frame<S: PageClass>(&self) -> PhysicalPage<S> {
        let paddr;
        if S::SIZE == 4096 {
            let addr = self.current.fetch_add(S::SIZE, Ordering::Relaxed);
            if addr >= self.max {
                // To hopefully avoid overflow from repeated use
                self.current.store(self.max, Ordering::Relaxed);
                panic!("Out of physical memory");
            }
            paddr = PAddr(addr);
        } else {
            (paddr, _) = self.alloc_range(S::SIZE, S::SIZE);
        }
        PhysicalPage {
            paddr: paddr.0,
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

    pub fn dealloc_frame<S: PageClass>(&self, page: PhysicalPage<S>) {
        let paddr = page.paddr;
        core::mem::forget(page);
        // TODO: actually free the page...
        let _ = paddr;
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
