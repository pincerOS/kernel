use alloc::collections::btree_map::BTreeMap;
//Page allocator will be accessible here while page fault handler is not ready
use crate::arch::memory::{MappingError, PAGE_ALLOCATOR, USER_PG_SZ, map_pa_to_va_user};
use crate::sync::SpinLock;

pub struct Process {
    ttbr0_el1: usize,
    memory_range_map: SpinLock<BTreeMap<usize, MemoryRangeNode>>
}

struct MemoryRangeNode {
    start: usize,
    size: usize
    //TODO: can add file descriptors here
}

impl Process {
    
    pub fn new(ttrb0: usize) -> Self {
        Process {ttrb0_el1: ttrb0, memory_range_map: SpinLock::new(BtreeMap::new())}
    }

    //TODO: add more params
    //TODO: add documentation
    pub fn reserve_memory_range(&self, mut start_addr: usize, mut size: usize) -> Result<usize, MappingError>{
        //aligning start_addr and size to page size 
        start_addr = (start_addr / USER_PG_SZ) * USER_PG_SZ;
        size = (size / USER_PG_SZ) * USER_PG_SZ;
        
        //This should ideally never be empty because of the stack of the first thread
        if !self.memory_range_map.is_empty() {
            
            //start_addr being zero indicates that a spot big enough needs to be found
            if start_addr == 0 {
                //TODO: correct this to have proper initial range start addr
                let mut prev_end: usize = 0;
                let mut foundSpot = true;
                //Trying to find a spot in the virtual memory range where the requested range can
                //fit
                for (next_start, node) in &self.memory_range_map {
                    if (next_start - prev_end) >= size {
                        start_addr = prev_end;
                        break;
                    }
                }

                //Stack should be last thing but can potentially add logic to check for space
                //between stack and end of memory
            
                if !foundSpot {
                    return Err(MappingError::RequestedSizeUnavailable);
                }

            } else {
            
                //TODO: double check the collision check logic for off by one errors
                //using Unbounded would clean this up but its currently experimental
                let mut existing_range = self.memory_range_map.range(0..start_addr);

                if let Some(entry) = existing_range.last() {
                    let rangeStart: usize = entry.1.start;
                    let rangeEnd: usize = rangeStart + entry.1.size;

                    if (rangeStart <= start_addr) && (start_addr < rangeEnd){
                        return Err(MappingError::MemoryRangeCollision);
                    }
                }
                
                //Getting start of the last node in the map
                let greatest_start: usize = *self.memory_range_map.iter().last().unwrap().0;
                existing_range = self.memory_range_map.range(start_addr..=greatest_start).peekable();

                //This should always exist
                let rangeStart: usize = existing_range.peek().1.start;
                let rangeEnd: usize = rangeStart + existing_range.peek().1.size;
                
                //end can be equal to start of next range
                if (rangeStart < (start_addr + size)) && ((start_addr + size) <= rangeEnd) {
                    return Err(MappingError::MemoryRangeCollision);
                }
            }

        }
        
        self.memory_range_map.insert(start_addr, MemoryRangeNode{start_addr, size});

        //TODO: remove this once page fault handler is set up

        //Temporary: allocate memory for the reserved range right away
        for virt_addr in (start_addr..(start_addr + size)) {
            let (_page_va, page_pa) = PAGE_ALLOCATOR.get().alloc_frame();
            unsafe { map_pa_to_va_user(page_pa, virt_addr, self.tttbr0_el1)?; }
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
