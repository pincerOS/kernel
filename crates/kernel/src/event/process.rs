use alloc::collections::btree_map::BTreeMap;
use crate::arch::memory::MappingError;

pub struct Process {
    pub ttbr0_el1: usize,
    memory_range_map: BTreeMap<usize, MemoryRangeNode>
}

struct MemoryRangeNode {
    start: usize,
    size: usize
    //TODO: can add file descriptors here
}

impl Process {
    //TODO: add more params
    //TODO: add documentation
    fn reserve_memory_range(&self, mut start_addr: usize, size: usize) -> Result<usize, MappingError>{

        
        //This should ideally never be empty because of the stack of the first thread
        if !self.memory_range_map.is_empty() {
            
            //start_addr being zero indicates that a spot big enough needs to be found
            if start_addr == 0 {

            } else {
                
                //TODO: move this collision check logic into a helper method, it can potentially be
                //used in the page fault handler
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

        //check if wanted range hits any current range
        //if its not reserved, reserve it but don't allocate
        //ideally this method will always be done for allocations so 
        //it shouldn't be necessary for this method to traverse
        //page tables
    }
}
