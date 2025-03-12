use alloc::collections::btree_map::BTreeMap;
//Page allocator will be accessible here while page fault handler is not ready
use crate::arch::memory::{MappingError, PAGE_ALLOCATOR, USER_PG_SZ, map_pa_to_va_user};

pub struct Process {
    ttbr0_el1: usize,
    memory_range_map: BTreeMap<usize, MemoryRangeNode>,
}

struct MemoryRangeNode {
    start: usize,
    size: usize
    //TODO: can add file descriptors here
}

impl Process {
    
    //TODO: implement drop for this to deallocate all pages

    pub fn new(ttbr0: usize) -> Self {
        Process {ttbr0_el1: ttbr0, memory_range_map: BTreeMap::new()}
    }

    //TODO: add more params
    //TODO: add documentation
    pub fn reserve_memory_range(&mut self, mut start_addr: usize, mut size: usize) -> Result<usize, MappingError>{
        //aligning start_addr and size to page size 
        start_addr = (start_addr / USER_PG_SZ) * USER_PG_SZ;
        size = (size / USER_PG_SZ) * USER_PG_SZ;
        
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

                    if (range_start <= start_addr) && (start_addr < range_end){
                        return Err(MappingError::MemoryRangeCollision);
                    }
                }
                
                //Getting start of the last node in the map
                let greatest_start: usize = *self.memory_range_map.iter().last().unwrap().0;
                let mut existing_range = self.memory_range_map.range(start_addr..=greatest_start).peekable();

                //This should always exist
                let range_start: usize = existing_range.peek().unwrap().1.start;
                let range_end: usize = range_start + existing_range.peek().unwrap().1.size;
                
                //end can be equal to start of next range
                if (range_start < (start_addr + size)) && ((start_addr + size) <= range_end) {
                    return Err(MappingError::MemoryRangeCollision);
                }
            }

        }
        
        self.memory_range_map.insert(start_addr, MemoryRangeNode{start: start_addr, size: size});
        return Ok(start_addr);
    }

    //TODO: remove this once page fault handler is set up
    //This is a temporary function to fill a mapped range with pages
    pub fn populate_range(&self, start_addr: usize, size: usize) -> () {
        
        //Temporary: allocate memory for the reserved range right away
        for virt_addr in start_addr..(start_addr + size) {
            //TODO: make another version of alloc frame which uses the physical base to give pages
            //instead of using the page table allocator
            let (_page_va, page_pa) = PAGE_ALLOCATOR.get().alloc_frame();
            unsafe { map_pa_to_va_user(page_pa, virt_addr, self.ttbr0_el1).unwrap(); }
        }
    }

    ///Used to check if a given address exists within a reserved memory range
    //TODO: add more documentation
    pub fn check_addr_in_mapping(&self, addr: usize) -> bool {
        if !self.memory_range_map.is_empty() {
            
            for (_, node) in &self.memory_range_map {
                if (node.start <= addr) && (addr < (node.start + node.size)){
                    return true;
                }
            }

        }

        return false;
    }
}
