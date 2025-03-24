use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::arch::memory::vmm::{MappingError, PAGE_ALLOCATOR, USER_PG_SZ};
use crate::sync::once_cell::BlockingOnceCell;
use crate::sync::SpinLock;

pub mod fd;

pub type ProcessRef = Arc<Process>;

#[derive(Clone)]
struct MemoryRangeNode {
    start: usize,
    size: usize, //Size of range
    file_descriptor_index: Option<u32>,
    is_physical: bool, //used if mapping a specific paddr to a specific vaddr
}

pub struct UserPageTable {
    pub table: Box<crate::arch::memory::vmm::UserTranslationTable>,
    pub phys_addr: usize,
    memory_range_map: BTreeMap<usize, MemoryRangeNode>, //key: start addr
}

pub struct FileDescriptorList {
    pub desc: Vec<Option<fd::ArcFd>>,
}

pub struct ExitStatus {
    pub status: u32,
}

pub struct Process {
    pub page_table: SpinLock<UserPageTable>,
    pub root: Option<fd::ArcFd>,
    pub file_descriptors: SpinLock<FileDescriptorList>,
    pub exit_code: Arc<BlockingOnceCell<ExitStatus>>,
}

impl Process {
    pub fn new() -> Self {
        let (_, table) = unsafe { crate::arch::memory::vmm::create_user_region() };
        let user_table_vaddr = (&*table as *const _ as *const ()).addr();
        let user_table_phys = crate::memory::physical_addr(user_table_vaddr).unwrap() as usize;

        let mut page_table = UserPageTable {
            table,
            phys_addr: user_table_phys,
            memory_range_map: BTreeMap::new(),
        };

        Process {
            page_table: SpinLock::new(page_table),
            root: None,
            file_descriptors: SpinLock::new(FileDescriptorList { desc: Vec::new() }),
            exit_code: Arc::new(BlockingOnceCell::new()),
        }
    }

    pub fn get_ttbr0(&self) -> usize {
        self.page_table.lock().phys_addr
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

        /*
        let dst_data = 0x20_0000 as *mut u8;
        let src_data = 0x20_0000 as *const u8;
        let src_size = 0x20_0000 * 7;
        */

        let old_page_dir = self.get_ttbr0();
        let new_page_dir = new_process.get_ttbr0();

        unsafe {
            let tmp_page_dir: usize;
            asm!("mrs {0}, TTBR0_EL1", out(reg) tmp_page_dir);
            if tmp_page_dir != old_page_dir {
                asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) old_page_dir);
            }
        }

        /*
        for i in 0..(src_size / buf_size) {
            unsafe {
                copy_nonoverlapping(src_data.byte_add(i * buf_size), buf_ptr, buf_size);
                asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) new_page_dir);
                copy_nonoverlapping(buf_ptr, dst_data.byte_add(i * buf_size), buf_size);
                asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) old_page_dir);
            }
        }
        */

        // /*
        //bad: locks other threads in the process from using this
        for (range_start, node) in &self.page_table.lock().memory_range_map {
            //this is mildly sus and will need to be changed
            new_process
                .page_table
                .lock()
                .reserve_memory_range(
                    *range_start,
                    node.size,
                    match node.file_descriptor_index {
                        None => u32::MAX,
                        Some(idx) => idx,
                    },
                    *range_start != 0x200_000,
                )
                .unwrap();

            if node.is_physical {
                new_process
                    .page_table
                    .lock()
                    .set_range_as_physical(*range_start);
            }

            let mut copy_idx: usize = 0;
            let copy_size = core::cmp::min(node.size, buf_size);

            let dst_data = *range_start as *mut u8;
            let src_data = *range_start as *const u8;

            while copy_idx < node.size {
                unsafe {
                    copy_nonoverlapping(src_data.byte_add(copy_idx), buf_ptr, copy_size);
                    asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) new_page_dir);
                    copy_nonoverlapping(buf_ptr, dst_data.byte_add(copy_idx), copy_size);
                    asm!("msr TTBR0_EL1, {0}", "dsb sy", "tlbi vmalle1is", "dsb sy", in(reg) old_page_dir);
                }

                copy_idx += copy_size;
            }
        }
        // */
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

impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            crate::arch::memory::vmm::clear_user_vaddr_space(&mut self.page_table.lock().table)
        };
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
    pub fn close(&mut self, idx: usize) -> Option<fd::ArcFd> {
        match self.desc.get_mut(idx) {
            Some(slot) => slot.take(),
            None => None,
        }
    }
}

impl UserPageTable {
    //TODO: add more params
    //TODO: add documentation
    pub fn reserve_memory_range(
        &mut self,
        mut start_addr: usize,
        req_size: usize,
        fd_index: u32,
        fill_pages: bool, //fill the mmapped range with pages
    ) -> Result<usize, MappingError> {
        //aligning start_addr and size to page size
        start_addr = (start_addr / USER_PG_SZ) * USER_PG_SZ;
        let mut size = (req_size / USER_PG_SZ) * USER_PG_SZ;
        if (req_size % USER_PG_SZ) != 0 {
            size += USER_PG_SZ;
        }

        //This should ideally never be empty because of the stack of the first thread
        if !self.memory_range_map.is_empty() {
            //start_addr being zero indicates that a spot big enough needs to be found
            if start_addr == 0 {
                //TODO: correct this to have proper initial range start addr
                let mut prev_end: usize = 0;
                let mut found_spot = false;
                //Trying to find a spot in the virtual memory range where the requested range can
                //fit
                for (next_start, node) in &self.memory_range_map {
                    if (next_start - prev_end) >= size {
                        start_addr = prev_end;
                        found_spot = true;
                        break;
                    }

                    prev_end = node.start + node.size;
                }

                //Stack should be last thing but can potentially add logic to check for space
                //between stack and end of memory

                if !found_spot {
                    return Err(MappingError::RequestedSizeUnavailable);
                }
            } else {
                //TODO: double check the collision check logic for off by one errors
                //using Unbounded would clean this up but its currently experimental
                let existing_range = self.memory_range_map.range(0..start_addr);
                if let Some(entry) = existing_range.last() {
                    let range_start: usize = entry.1.start;
                    let range_end: usize = range_start + entry.1.size;

                    if (range_start <= start_addr) && (start_addr < range_end) {
                        return Err(MappingError::MemoryRangeCollision);
                    }
                }
                //Getting start of the last node in the map
                let last_elem_iter = self.memory_range_map.iter().last().unwrap();
                let greatest_start: usize = *last_elem_iter.0;
                let greatest_end: usize = greatest_start + last_elem_iter.1.size;

                if (greatest_start < start_addr) && (start_addr < greatest_end) {
                    return Err(MappingError::MemoryRangeCollision);
                } else if start_addr < greatest_start {
                    let mut existing_range = self
                        .memory_range_map
                        .range(start_addr..=greatest_start)
                        .peekable();

                    //This should always exist
                    let range_start: usize = existing_range.peek().unwrap().1.start;
                    let range_end: usize = range_start + existing_range.peek().unwrap().1.size;

                    //end can be equal to start of next range
                    if (range_start < (start_addr + size)) && ((start_addr + size) <= range_end) {
                        return Err(MappingError::MemoryRangeCollision);
                    }
                }
            }
        }

        self.memory_range_map.insert(
            start_addr,
            MemoryRangeNode {
                start: start_addr,
                size: size,
                file_descriptor_index: if fd_index == u32::MAX {
                    None
                } else {
                    Some(fd_index)
                },
                is_physical: false,
            },
        );

        if fill_pages {
            self.populate_range(start_addr, size);
        }

        //This could be a lower number than what is requested because this value is page aligned
        return Ok(start_addr);
    }

    //Sets a mapped range as a physical address range, so the memory behind it won't be freed
    //This exists for now to be used with the initial user space
    pub fn set_range_as_physical(&mut self, mut addr: usize) -> () {
        addr = (addr / USER_PG_SZ) * USER_PG_SZ;
        if let Some(node_ref) = self.memory_range_map.get_mut(&addr) {
            node_ref.is_physical = true;
        }
    }

    //Fills a mapped range with pages
    //It currently doesn't do any error checking
    //This is going to be used until the page fault handler is ready
    pub fn populate_range(&mut self, mut start_addr: usize, req_size: usize) -> () {
        //aligning start_addr and size to page size
        start_addr = (start_addr / USER_PG_SZ) * USER_PG_SZ;
        let mut size = (req_size / USER_PG_SZ) * USER_PG_SZ;
        if (req_size % USER_PG_SZ) != 0 {
            size += USER_PG_SZ;
        }

        //Temporary: allocate memory for the reserved range right away
        for virt_addr in (start_addr..(start_addr + size)).step_by(USER_PG_SZ) {
            //TODO: make another version of alloc frame which uses the physical base to give pages
            //instead of using the page table allocator
            let (_page_va, page_pa) = PAGE_ALLOCATOR.get().alloc_frame();
            unsafe {
                crate::arch::memory::vmm::map_pa_to_va_user(page_pa, virt_addr, &mut self.table)
                    .unwrap();
            }
        }
    }

    ///Maps a range of phsical addresses to a previously reserved range of virtual addresses
    pub fn map_to_physical_range(
        &mut self,
        mut start_va: usize,
        mut start_pa: usize,
    ) -> Result<(), MappingError> {
        start_va = (start_va / USER_PG_SZ) * USER_PG_SZ;
        start_pa = (start_pa / USER_PG_SZ) * USER_PG_SZ;

        let size: usize;
        match self.memory_range_map.get_mut(&start_va) {
            Some(node) => {
                size = node.size;
                node.is_physical = true;
            }
            None => return Err(MappingError::NotInMemoryRange),
        }

        for increment in (0..size).step_by(USER_PG_SZ) {
            unsafe {
                crate::arch::memory::vmm::map_pa_to_va_user(
                    start_pa + increment,
                    start_va + increment,
                    &mut self.table,
                )?;
            }
        }

        return Ok(());
    }

    ///Removes a node from the memory range mapping and deallocates all pages in it
    ///Freeing the page is the responsibility of this function in the case that a mapped range had
    ///specific physical addresses mapped to it
    pub fn unmap_memory_range(&mut self, req_addr: usize) -> Result<(), MappingError> {
        let range_start: usize;
        let range_end: usize;
        let is_physical: bool;

        {
            let range_node: &MemoryRangeNode = self.get_memory_range_node(req_addr)?;
            range_start = range_node.start;
            range_end = range_start + range_node.size;
            is_physical = range_node.is_physical;
        }

        for addr in (range_start..range_end).step_by(USER_PG_SZ) {
            unsafe {
                match crate::arch::memory::vmm::unmap_va_user(addr, &mut self.table) {
                    Ok(val) => {
                        if !is_physical {
                            //TODO: free the page here
                        }
                    }
                    //These two can do nothing, just means that page was never mapped/used
                    Err(MappingError::TableDescriptorNotValid) => {}
                    Err(MappingError::LeafTableSpotNotValid) => {}
                    //TODO: once huge page support is added update this
                    Err(e) => println!("Error: {}", e),
                }
            };
        }

        self.memory_range_map.remove(&range_start);
        return Ok(());
    }

    fn get_memory_range_node(&self, addr: usize) -> Result<&MemoryRangeNode, MappingError> {
        if !self.memory_range_map.is_empty() {
            let existing_range = self.memory_range_map.range(0..=addr);

            if let Some(entry) = existing_range.last() {
                let range_start: usize = entry.1.start;
                let range_end: usize = range_start + entry.1.size;

                if (range_start <= addr) && (addr < range_end) {
                    return Ok(&entry.1);
                }
            }
        }

        return Err(MappingError::NotInMemoryRange);
    }

    ///Returns the start of the memory range associated with an address
    ///This can be used to check if an address exists within a mapped range
    pub fn get_memory_range_start(&self, addr: usize) -> Result<usize, MappingError> {
        if !self.memory_range_map.is_empty() {
            let existing_range = self.memory_range_map.range(0..=addr);

            if let Some(entry) = existing_range.last() {
                let range_start: usize = entry.1.start;
                let range_end: usize = range_start + entry.1.size;

                if (range_start <= addr) && (addr < range_end) {
                    return Ok(range_start);
                }
            }
        }

        return Err(MappingError::NotInMemoryRange);
    }

    pub fn clear_address_space(&mut self) -> () {
        let curr_map: &mut BTreeMap<usize, MemoryRangeNode> = &mut self.memory_range_map;
        for (range_start, node) in &mut *curr_map {
            let range_end = range_start + node.size;
            let is_physical: bool = node.is_physical;
            for addr in (*range_start..range_end).step_by(USER_PG_SZ) {
                unsafe {
                    match crate::arch::memory::vmm::unmap_va_user(addr, &mut self.table) {
                        Ok(val) => {
                            if !is_physical {
                                //TODO: free the page here
                            }
                        }
                        //These two can do nothing, just means that page was never mapped/used
                        Err(MappingError::TableDescriptorNotValid) => {}
                        Err(MappingError::LeafTableSpotNotValid) => {}
                        Err(MappingError::HugePagePresent) => {
                            if !is_physical {
                                //TODO: once huge page support is added update this
                            }
                        }
                        Err(e) => println!("Error: {}", e),
                    }
                };
            }
        }

        curr_map.clear();
    }
}
