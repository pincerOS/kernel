use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;

use crate::arch::memory::machine::{LeafDescriptor, TranslationDescriptor};
use crate::arch::memory::palloc::{Size4KiB, PAGE_ALLOCATOR};
use crate::arch::memory::table::PageTablePtr;
use crate::arch::memory::vmm::{
    alloc_top_page_table, get_translation_descriptor, set_translation_descriptor, MappingError,
    PAGE_SIZE, USER_PG_SZ,
};
use crate::event::async_handler::{run_async_handler, HandlerContext};
use crate::event::context::{deschedule_thread, Context, DescheduleAction};
use crate::event::exceptions::DataAbortISS;

use crate::process::fd::FileDescriptor;
use crate::syscall::fb_hack::MemFd;

use super::fd::ArcFd;

#[derive(Debug)]
pub enum MmapError {
    MemoryRangeCollision,
    NoSuchEntry,
    RequestedSizeUnavailable,
    FileError,
}

pub struct UserAddrSpace {
    table: PageTablePtr,
    memory_range_map: BTreeMap<usize, MemoryRangeNode>, //key: start addr
}

#[derive(Clone)]
pub struct MemoryRangeNode {
    pub start: usize,
    pub size: usize,
    pub kind: MappingKind,
}

#[derive(Clone)]
pub enum MappingKind {
    Anon,
    File(ArcFd),
}


impl UserAddrSpace {
    pub fn new() -> Self {
        let table = alloc_top_page_table();

        Self {
            table,
            memory_range_map: BTreeMap::new(),
        }
    }

    pub fn get_ttbr0(&self) -> usize {
        self.table.paddr()
    }

    pub async fn fork(&self) -> Self {
        use core::arch::asm;
        use core::mem::MaybeUninit;
        use core::ptr::copy_nonoverlapping;

        let mut new_mem = Self::new();

        // This is a massive hack
        let buf_size = 1 << 16;
        let mut buffer: Box<[MaybeUninit<u8>]> = Box::new_uninit_slice(buf_size);

        let old_page_dir = self.get_ttbr0();
        let new_page_dir = new_mem.get_ttbr0();

        let active_page_dir: usize;
        unsafe {
            asm!("mrs {0}, TTBR0_EL1", out(reg) active_page_dir);
            if active_page_dir != old_page_dir {
                asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) old_page_dir);
            }
        }

        for (range_start, node) in &self.memory_range_map {
            let start = new_mem
                .insert_vme_at(node.start, node.size, node.kind.clone())
                .unwrap();
            assert!(start == *range_start);
            
            for offset in (0..node.size).step_by(buf_size) {
                let chunk_size = (node.size - offset).min(buf_size);
                new_mem
                    .populate_range(node, node.start + offset, chunk_size)
                    .await
                    .unwrap();
                // TODO: don't populate source, skip copying unloaded data
                self.populate_range(node, node.start + offset, chunk_size)
                    .await
                    .unwrap();
               
                //Don't want to copy data in shared mem
                if let MappingKind::File(arc) = &node.kind {
                     
                    if let Some(_s) = arc.as_any().downcast_ref::<MemFd>() {
                        println!("Skipping over shared range the right way");
                        continue;
                    }
                    
                }
                 
                let src_data = node.start as *const u8;
                let dst_data = node.start as *mut u8;
                let buf_ptr: *mut u8 = buffer.as_mut_ptr().cast();
                unsafe {
                    copy_nonoverlapping(src_data.byte_add(offset), buf_ptr, chunk_size);
                    asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) new_page_dir);
                    copy_nonoverlapping(buf_ptr, dst_data.byte_add(offset), chunk_size);
                    asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) old_page_dir);
                }
            }
        }

        if active_page_dir != old_page_dir {
            unsafe {
                asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) active_page_dir);
            }
        }

        new_mem
    }

    pub fn insert_vme_at(
        &mut self,
        start: usize,
        size: usize,
        kind: MappingKind,
    ) -> Result<usize, MmapError> {
        let start_addr = (start / PAGE_SIZE) * PAGE_SIZE;
        let size_pages = (size + (start - start_addr)).next_multiple_of(PAGE_SIZE);

        if let Some((_, last_before)) = self.memory_range_map.range(0..start_addr).last() {
            if last_before.start + last_before.size > start_addr {
                return Err(MmapError::MemoryRangeCollision);
            }
        }
        if let Some((_, first_after)) = self.memory_range_map.range(start_addr..).next() {
            if start_addr + size_pages > first_after.start {
                return Err(MmapError::MemoryRangeCollision);
            }
        }
        
        //TODO: take another look at the size here
        let node = MemoryRangeNode { start, size, kind };
        self.memory_range_map.insert(start, node);
        Ok(start_addr)
    }

    pub fn find_vme_space(&mut self, size: usize) -> Result<usize, MmapError> {
        let size = size.next_multiple_of(PAGE_SIZE);

        let mut prev_end = 4096; // Don't map the null page...
        for (_, node) in self.memory_range_map.range(prev_end..) {
            if node.start - prev_end >= size {
                return Ok(prev_end);
            }
            prev_end = node.start + node.size;
        }

        let space_end = 1 << 48;
        if space_end - prev_end >= size {
            Ok(prev_end)
        } else {
            Err(MmapError::RequestedSizeUnavailable)
        }
    }

    pub fn mmap(
        &mut self,
        start_addr: Option<usize>,
        size: usize,
        kind: MappingKind,
    ) -> Result<usize, MmapError> {
        let start_addr = match start_addr {
            Some(s) => s,
            None => self.find_vme_space(size)?,
        };
        let base_addr = self.insert_vme_at(start_addr, size, kind)?;
        Ok(base_addr)
    }

    // Addr must be the start of a VME
    pub fn unmap(&mut self, addr: usize) -> Result<(), MmapError> {
        let vme = self
            .memory_range_map
            .remove(&addr)
            .ok_or(MmapError::NoSuchEntry)?;

        // TODO: only unmap allocated pages
        for virt_addr in (vme.start..(vme.start + vme.size)).step_by(USER_PG_SZ) {
            let cur = unsafe { get_translation_descriptor(self.table, virt_addr, 3, 0) };

            let desc = match cur {
                Ok(desc) => desc,
                Err(MappingError::HugePagePresent) => {
                    todo!("Unmapping ranges with huge pages?")
                }
                Err(MappingError::LevelEntryUnset(_)) => continue,
                Err(_) => todo!(),
            };

            let leaf = unsafe { desc.leaf };
            if leaf.is_valid() {
                let new_desc = TranslationDescriptor::unset();
                unsafe {
                    set_translation_descriptor(self.table, virt_addr, 3, 0, new_desc, false)
                        .unwrap()
                }
                // TODO: free it
                match &vme.kind {
                    MappingKind::Anon => {
                        // TODO: free
                    }
                    MappingKind::File(_arc) => {
                        // TODO: notify file that it's unused?
                        // (for ref counts, page cache?)
                    }
                }
            }
        }

        // TODO: flush?
        // TODO: don't flush in each individual set_descriptor

        Ok(())
    }

    pub fn get_vme(&self, addr: usize) -> Option<&MemoryRangeNode> {
        let existing_range = self.memory_range_map.range(0..=addr);
        if let Some((_, entry)) = existing_range.last() {
            if entry.start <= addr && entry.start + entry.size > addr {
                return Some(entry);
            }
        }
        None
    }

    pub async fn populate_page(
        &self,
        vme: &MemoryRangeNode,
        vaddr: usize,
    ) -> Result<(), MmapError> {
        assert!(vaddr % PAGE_SIZE == 0);

        if let Ok(desc) = unsafe { get_translation_descriptor(self.table, vaddr, 3, 0) } {
            if unsafe { desc.leaf }.is_valid() {
                return Ok(());
            }
        }

        let desc = match &vme.kind {
            MappingKind::Anon => {
                let page = PAGE_ALLOCATOR.get().alloc_frame::<Size4KiB>();
                let desc = LeafDescriptor::new(page.paddr)
                    .union(LeafDescriptor::UNPRIVILEGED_ACCESS)
                    .difference(LeafDescriptor::UXN)
                    .set_global();

                // TODO: zero the page...

                desc
            }
            MappingKind::File(fd) => {
                let offset = vaddr - vme.start;
                let page = match fd.mmap_page(offset as u64).await.map(|r| r.as_result()) {
                    Some(Ok(page)) => page,
                    Some(Err(_e)) => {
                        println!("Error in mmap page");
                        return Err(MmapError::FileError);
                    },
                    None => { 
                        println!("Mmap page returned none");
                        return Err(MmapError::FileError);
                    },
                };
                let desc = LeafDescriptor::new(page as usize)
                    .union(LeafDescriptor::UNPRIVILEGED_ACCESS)
                    .difference(LeafDescriptor::UXN)
                    .set_global();
                desc
            }
        };

        unsafe {
            set_translation_descriptor(self.table, vaddr, 3, 0, desc.into(), true).unwrap();
        }

        Ok(())
    }

    pub async fn populate_range(
        &self,
        vme: &MemoryRangeNode,
        start: usize,
        len: usize,
    ) -> Result<(), MmapError> {
        for off in (start..start + len).step_by(PAGE_SIZE) {
            self.populate_page(vme, off).await?;
        }
        Ok(())
    }

    pub fn clear_address_space(&mut self) {
        let mut cur = 0;
        while let Some((start, node)) = self.memory_range_map.range(cur..).next() {
            cur = start + node.size;
            self.unmap(*start).unwrap();
        }
        assert!(self.memory_range_map.is_empty());
    }
}

impl Drop for UserAddrSpace {
    fn drop(&mut self) {
        self.clear_address_space();
    }
}

unsafe impl Send for UserAddrSpace {}
unsafe impl Sync for UserAddrSpace {}

pub fn page_fault_handler(ctx: &mut Context, far: usize, _iss: DataAbortISS) -> *mut Context {
    run_async_handler(ctx, async move |mut context: HandlerContext<'_>| {
        let proc = context.cur_process().unwrap();

        // TODO: make sure misaligned loads don't loop here?
        let page_addr = (far / PAGE_SIZE) * PAGE_SIZE;

        let mem = proc.mem.lock();
        let vme = mem.get_vme(page_addr);
        match vme {
            None => {
                let exit_code = &proc.exit_code;
                exit_code.set(crate::process::ExitStatus {
                    status: -1i32 as u32,
                });
                drop(mem);

                println!("Invalid user access at addr {far:#10x}");
                println!("{:#?}", &*context.regs());

                let thread = context.detach_thread();
                unsafe { deschedule_thread(DescheduleAction::FreeThread, Some(thread)) }
            }
            Some(vme) => {
                mem.populate_page(vme, page_addr).await.unwrap(); // TODO: errors?
                drop(mem);
                context.resume_final()
            }
        }
    })
}
