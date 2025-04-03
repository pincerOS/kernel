use alloc::boxed::Box;

use crate::arch::memory::machine::LeafDescriptor;
use crate::arch::memory::palloc::PAGE_ALLOCATOR;
use crate::arch::memory::table::PageTablePtr;
use crate::arch::memory::vmm::{alloc_top_page_table, set_translation_descriptor};

pub struct UserAddrSpace {
    table: PageTablePtr,
}

impl UserAddrSpace {
    pub fn new() -> Self {
        let table = alloc_top_page_table();

        let virt_region_base = 0x20_0000;
        let region_size = 0x20_0000 * 7;
        let (phys_base, _) = PAGE_ALLOCATOR.get().alloc_range(region_size, 0x20_0000);

        // TODO: proper mappings
        let root_region_size = 0x20_0000; // 2 MiB
        for i in 0..7 {
            let phys_frame = phys_base.0 + root_region_size * i;
            let leaf = LeafDescriptor::new(phys_frame)
                .union(LeafDescriptor::UNPRIVILEGED_ACCESS)
                .difference(LeafDescriptor::UXN)
                .set_global()
                .difference(LeafDescriptor::IS_PAGE_DESCRIPTOR);
            println!(
                "map phys {:#010x} to virt {:#010x} for user ({i})",
                phys_frame,
                root_region_size * (i + 1)
            );
            unsafe {
                set_translation_descriptor(
                    table,
                    virt_region_base + root_region_size * i,
                    2,
                    0,
                    leaf.into(),
                    true,
                )
                .unwrap();
            }
        }

        Self { table }
    }

    pub fn get_ttbr0(&self) -> usize {
        self.table.paddr()
    }

    pub fn fork(&self) -> Self {
        use core::arch::asm;
        use core::mem::MaybeUninit;
        use core::ptr::copy_nonoverlapping;

        // This is a massive hack
        let buf_size = 1 << 16;
        let mut buffer: Box<[MaybeUninit<u8>]> = Box::new_uninit_slice(buf_size);
        let buf_ptr: *mut u8 = buffer.as_mut_ptr().cast();

        let dst_data = 0x20_0000 as *mut u8;
        let src_data = 0x20_0000 as *const u8;
        let src_size = 0x20_0000 * 7;

        let new_mem = Self::new();

        let old_page_dir = self.get_ttbr0();
        let new_page_dir = new_mem.get_ttbr0();

        unsafe {
            let tmp_page_dir: usize;
            asm!("mrs {0}, TTBR0_EL1", out(reg) tmp_page_dir);
            if tmp_page_dir != old_page_dir {
                asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) old_page_dir);
            }
        }

        for i in 0..(src_size / buf_size) {
            unsafe {
                copy_nonoverlapping(src_data.byte_add(i * buf_size), buf_ptr, buf_size);
                asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) new_page_dir);
                copy_nonoverlapping(buf_ptr, dst_data.byte_add(i * buf_size), buf_size);
                asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) old_page_dir);
            }
        }

        new_mem
    }
}

unsafe impl Send for UserAddrSpace {}
unsafe impl Sync for UserAddrSpace {}
