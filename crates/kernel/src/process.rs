use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::sync::once_cell::BlockingOnceCell;
use crate::sync::SpinLock;

pub mod fd;

pub type ProcessRef = Arc<Process>;

pub struct UserPageTable {
    pub table: Box<crate::arch::memory::vmm::UnifiedTranslationTable>,
    pub phys_addr: usize,
}

pub struct FileDescriptorList {
    pub desc: Vec<Option<fd::ArcFd>>,
}

pub struct ExitStatus {
    pub status: u32,
}

pub struct Process {
    pub page_table: UserPageTable,
    pub root: Option<fd::ArcFd>,
    pub file_descriptors: SpinLock<FileDescriptorList>,
    pub exit_code: Arc<BlockingOnceCell<ExitStatus>>,
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

        Process {
            page_table,
            root: None,
            file_descriptors: SpinLock::new(FileDescriptorList { desc: Vec::new() }),
            exit_code: Arc::new(BlockingOnceCell::new()),
        }
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

        {
            let old_fds = self.file_descriptors.lock();
            let mut new_fds = new_process.file_descriptors.lock();
            for (idx, desc) in old_fds
                .desc
                .iter()
                .enumerate()
                .filter_map(|(idx, desc)| Some((idx, desc.as_ref()?)))
            {
                let _ = new_fds.set(idx, desc.clone());
            }
        }

        new_process
    }
}

impl FileDescriptorList {
    pub fn get(&self, idx: usize) -> Option<&fd::ArcFd> {
        self.desc.get(idx).and_then(|s| s.as_ref())
    }
    #[must_use]
    pub fn set(&mut self, idx: usize, descriptor: fd::ArcFd) -> Option<fd::ArcFd> {
        match self.desc.get_mut(idx) {
            Some(slot) => slot.replace(descriptor),
            None => {
                // TODO: this is an easy DOS vector
                self.desc.resize(idx + 1, None);
                self.desc[idx] = Some(descriptor);
                None
            }
        }
    }
    pub fn insert(&mut self, descriptor: fd::ArcFd) -> usize {
        for (i, slot) in self.desc.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(descriptor);
                return i;
            }
        }
        let idx = self.desc.len();
        self.desc.push(Some(descriptor));
        idx
    }
    #[must_use]
    pub fn remove(&mut self, idx: usize) -> Option<fd::ArcFd> {
        match self.desc.get_mut(idx) {
            Some(slot) => slot.take(),
            None => None,
        }
    }
}
