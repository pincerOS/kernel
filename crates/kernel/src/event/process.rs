use alloc::collections::btree_map::BTreeMap;
//Page allocator will be accessible here while page fault handler is not ready
use crate::arch::memory::{
    clear_user_vaddr_space, map_pa_to_va_user, unmap_va_user, MappingError, PAGE_ALLOCATOR,
    USER_PG_SZ,
};

pub struct Process {
    ttbr0_el1: usize,
    memory_range_map: BTreeMap<usize, MemoryRangeNode>,
}

struct MemoryRangeNode {
    start: usize,
    size: usize,       //TODO: can add file descriptors here
    is_physical: bool, //used if mapping a specific paddr to a specific vaddr
}

impl Process {
    pub fn new(ttbr0: usize) -> Self {
        Process {
            ttbr0_el1: ttbr0,
            memory_range_map: BTreeMap::new(),
        }
    }

    //TODO: add more params
    //TODO: add documentation
    pub fn reserve_memory_range(
        &mut self,
        mut start_addr: usize,
        req_size: usize,
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
    pub fn populate_range(&self, mut start_addr: usize, req_size: usize) -> () {
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
            println!("Mapping pa {} to va {}", page_pa, virt_addr);
            println!("ttbr0 value: {:x}", self.ttbr0_el1);
            unsafe {
                map_pa_to_va_user(page_pa, virt_addr, self.ttbr0_el1).unwrap();
            }
        }
    }

    ///Removes a node from the memory range mapping and deallocates all pages in it
    ///Freeing the page is the responsibility of this function in the case that a mapped range had
    ///specific physical addresses mapped to it
    pub fn unmap_memory_range(&mut self, req_addr: usize) -> Result<(), MappingError> {
        let range_node: &MemoryRangeNode = self.get_memory_range_node(req_addr)?;
        let range_start: usize = range_node.start;
        let range_end: usize = range_start + range_node.size;

        for addr in (range_start..range_end).step_by(USER_PG_SZ) {
            unsafe {
                match unmap_va_user(addr, self.ttbr0_el1) {
                    Ok(val) => {
                        if !range_node.is_physical {
                            //TODO: free the page here
                        }
                        //TODO: invalidate TLB entry
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
}

impl Drop for Process {
    fn drop(&mut self) {
        unsafe { clear_user_vaddr_space(self.ttbr0_el1) };
    }
}
