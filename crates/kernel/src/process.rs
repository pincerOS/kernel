use alloc::boxed::Box;
use alloc::sync::Arc;

pub type ProcessRef = Arc<Process>;

pub struct UserPageTable {
    pub table: Box<crate::arch::memory::vmm::UserTranslationTable>,
    phys_addr: usize,
}

pub struct Process {
    pub page_table: UserPageTable,
}

impl Process {
    pub fn new() -> Self {
        let (_, table) = unsafe { crate::arch::memory::vmm::create_user_region() };
        let user_table_vaddr = (&*table as *const _ as *const ()).addr();
        let user_table_phys = crate::memory::physical_addr(user_table_vaddr).unwrap() as usize;

        let page_table = UserPageTable {
            table,
            phys_addr: user_table_phys,
        };

        Process { page_table }
    }

    pub fn get_ttbr0(&self) -> usize {
        self.page_table.phys_addr
    }

    pub fn fork(&self) -> Process {
        use core::arch::asm;
        use core::mem::MaybeUninit;
        use core::ptr::copy_nonoverlapping;

        // This is a massive hack
        let buf_size = 1 << 16;
        let mut buffer: Box<[MaybeUninit<u8>]> = Box::new_uninit_slice(buf_size);
        let buf_ptr: *mut u8 = buffer.as_mut_ptr().cast();

        let new_process = Process::new();

        let dst_data = 0x20_0000 as *mut u8;
        let src_data = 0x20_0000 as *const u8;
        let src_size = 0x20_0000 * 7;

        let old_page_dir = self.get_ttbr0();
        let new_page_dir = new_process.get_ttbr0();

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

        new_process
    }
}
