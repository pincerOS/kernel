use alloc::boxed::Box;
use core::{
    arch::asm,
    fmt::{Display, Formatter},
    ptr::{addr_of, NonNull},
};

use super::{
    machine::{LeafDescriptor, TableDescriptor, TranslationDescriptor},
    palloc::{PAddr, PhysicalPage, PAGE_ALLOCATOR},
    physical_addr,
};

const PG_SZ: usize = 0x1000;
const TRANSLATION_TABLE_SIZE: usize = PG_SZ / size_of::<TranslationDescriptor>();

#[repr(C, align(4096))]
pub struct UnifiedTranslationTable(pub [TranslationDescriptor; TRANSLATION_TABLE_SIZE]);

#[unsafe(no_mangle)]
static mut KERNEL_TRANSLATION_TABLE: UnifiedTranslationTable = UnifiedTranslationTable(
    [TranslationDescriptor {
        table: TableDescriptor::empty(),
    }; TRANSLATION_TABLE_SIZE],
);

#[allow(improper_ctypes)]
unsafe extern "C" {
    pub static mut __rpi_virt_base: ();
    pub static mut __rpi_phys_binary_start_addr: ();
    pub static mut __rpi_virt_binary_start_addr: ();
    pub static mut __rpi_phys_binary_end_addr: ();
    pub static mut __rpi_virt_binary_end_addr: ();
}

fn virt_addr_base() -> NonNull<()> {
    NonNull::new(&raw mut __rpi_virt_base).unwrap()
}

pub unsafe fn init_physical_alloc() {
    // TODO: proper physical memory layout documentation
    // Assume 1 GiB available; TODO: discover memory topology
    let base = 0x20_0000 * 16;
    let end = 1 << 30;
    unsafe { super::palloc::init_physical_alloc(base, end) };
}

pub fn create_user_table(phys_base: PAddr) -> alloc::boxed::Box<UnifiedTranslationTable> {
    let mut table = alloc::boxed::Box::new(UnifiedTranslationTable(
        [TranslationDescriptor {
            table: TableDescriptor::empty(),
        }; TRANSLATION_TABLE_SIZE],
    ));

    let table_ptr: *mut UnifiedTranslationTable = &mut *table as *mut UnifiedTranslationTable;
    
    let root_region_size = 0x20_0000; // 2 MiB
    let virt_region_base = 0x20_0000;
    for i in 1..8 {
        let phys_frame = phys_base.0 + root_region_size * i;
        let leaf = LeafDescriptor::new(phys_frame)
            // .clear_pxn()
            .union(LeafDescriptor::UNPRIVILEGED_ACCESS)
            .difference(LeafDescriptor::UXN)
            .set_global()
            .difference(LeafDescriptor::IS_PAGE_DESCRIPTOR);
        println!(
            "map phys {:#010x} to virt {:#010x} for user ({i})",
            phys_frame,
            root_region_size * (i + 1)
        );

        let translation_descriptor = TranslationDescriptor{ leaf: leaf };
        map_pa_to_va(phys_frame, virt_region_base + root_region_size * i, table_ptr, true).unwrap();
    }

    return table;
}

pub unsafe fn create_user_region() -> (*mut [u8], Box<UnifiedTranslationTable>) {
    let virt_region_base = 0x20_0000;
    let region_size = 0x20_0000 * 7;

    let (phys_base, _) = PAGE_ALLOCATOR.get().alloc_range(region_size, 0x20_0000);

    let user_table = create_user_table(phys_base);
    let user_table_vaddr = (&*user_table as *const UnifiedTranslationTable).addr();
    let user_table_phys = physical_addr(user_table_vaddr).unwrap();
    println!("creating user table, {:#010x}", user_table_phys);

    let ptr = core::ptr::with_exposed_provenance_mut::<u8>(virt_region_base);
    let slice = core::ptr::slice_from_raw_parts_mut(ptr, region_size);
    (slice, user_table)
}

/// only call once!
pub unsafe fn init() {

    let kernel_translation_table: *mut UnifiedTranslationTable = &mut KERNEL_TRANSLATION_TABLE as *mut UnifiedTranslationTable;
    unsafe {
        
        for i in 1..14 {
            //let translation_descriptor = TranslationDescriptor { leaf: LeafDescriptor::new(0x20_0000 * i).set_global().difference(LeafDescriptor::IS_PAGE_DESCRIPTOR)  };
            //set_translation_descriptor(0x20_0000 * i, 2, 0, );
            map_pa_to_va(0x20_0000 * i, 0x20_0000, kernel_translation_table, true).unwrap();
        }
        
        //No leaf table
    }
}

unsafe fn first_unused_virt_page_addr(table: *mut UnifiedTranslationTable, curr_level: u8) -> Option<usize> {
    assert!(curr_level <= 3);
    if curr_level == 3 {
        let table_base = table.cast::<LeafDescriptor>();
        for idx in 0..TRANSLATION_TABLE_SIZE {
            let entry = table_base.wrapping_add(idx);
            if !(unsafe { entry.read()  }.is_valid()) {
                return Some(idx * PG_SZ);
            }
        }
    } else {
        //TODO: double check the mov amount math
        let mov_amt: usize = PG_SZ * TRANSLATION_TABLE_SIZE.powi(3 - curr_level);
        let table_base = table.cast::<TableDescriptor>();
        for idx in 0..TRANSLATION_TABLE_SIZE {
            let entry = table_base.wrapping_add(idx);
            let mut descriptor = unsafe { entry.read() };

            if !descriptor.is_valid() {
                let frame = PAGE_ALLOCATOR.get().alloc_mapped_frame();
                descriptor = TableDescriptor::new(frame.paddr);
                let intermediate_descriptor = TranslationDescriptor { table: descriptor, };
                unsafe { entry.write(intermediate_descriptor) };
            }
            
            let next_table: *mut UnifiedTranslationTable = (descriptor.get_pa() << 12) as *mut UnifiedTranslationTable;
            if let Some(addr) = first_unused_virt_page_addr(next_table, curr_level + 1){
                return Some((idx * mov_amt) + addr);
            }

        }
    }

    return None;
}

/*
unsafe fn first_unused_virt_page(table: *mut UnifiedTranslationTable) -> Option<usize> {
    
    let table_base = table.cast::<LeafDescriptor>();
    for idx in 0..KERNEL_LEAF_TABLE_SIZE {
        let entry = table_base.wrapping_add(idx);
        let desc = unsafe { entry.read() };
        if !desc.is_valid() {
            return Some(idx);
        }
    }
    
    None
}
*/

//Address levels are taken from the following documentation
//https://developer.arm.com/documentation/101811/0104/Translation-granule/The-starting-level-of-address-translation
#[derive(Debug)]
pub enum MappingError {
    HugePagePresent,
    TableDescriptorPresent,
    LeafTableSpotTaken,
    LevelEntryUnset(u8),
}

impl Display for MappingError {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            Self::HugePagePresent => write!(f, "Huge page present is present in desired mapping location"),
            Self::TableDescriptorPresent => write!(f, "Table descriptor present in desired mapping location"),
            Self::LeafTableSpotTaken => write!(f, "The spot in the leaf table that is being mapped to is already taken"),
            Self::LevelEntryUnset(value) => write!(f, "The level {} entry for this address is invalid", value),
        }
    }
}


pub unsafe fn get_translation_descriptor(va: usize, target_level: u8, curr_level: u8, translation_table: *mut UnifiedTranslationTable) -> Result<TranslationDescriptor, MappingError> {
    assert!(target_level <= 3);
    assert!(curr_level <= 3);
    //12 bits for 4096 byte page offset, after that its 9 bits for each level
    let mask = 0b111111111;
    let table_index = (va >> (12 + (9 * (3 - curr_level)))) & mask;
    let table_entry: *mut TranslationDescriptor = translation_table.cast::<TranslationDescriptor>().wrapping_add(table_index);
    
    if curr_level == target_level {
        let descriptor: TranslationDescriptor = unsafe { table_entry.read() };
        return OK();
    }

    if unsafe { table_entry.read()  }.is_valid() {
        return Err(MappingError::LevelEntryUnset(curr_level));
    } else {
        let table_entry: *mut TableDescriptor = table_entry.cast::<TableDescriptor>();
        let next_lvl_table_pa: usize = unsafe { table_entry.read()  }.get_pa() << 12;
        let next_lvl_table: *mut UnifiedTranslationTable = PAGE_ALLOCATOR.get().get_mapped_frame(PhysicalPage::new(PAddr(next_level_table_pa))).cast::<KernelTranslationTable>();

        table_desc = get_table_descriptor_user(va, target_level, curr_level + 1, next_lvl_table)?;
        return Ok(table_desc);
    }
}

pub unsafe fn set_translation_descriptor(va: usize, target_level: u8, curr_level: u8, translation_table: *mut UnifiedTranslationTable, descriptor: TranslationDescriptor) -> Result<(), MappingError> {
    assert!(target_level <= 3);
    assert!(curr_level <= 3);
    let mask = 0b111111111;
    let table_index = (va >> (12 + (9 * (3 - curr_level)))) & mask;
    let table_entry: *mut TranslationDescriptor = translation_table.cast::<TranslationDescriptor>().wrapping_add(table_index);

    if curr_level == target_level {
        unsafe { 
            table_entry_ptr.write(descriptor); 
            
            asm! { "dsb ISH", options(readonly, nostack, preserves_flags) }
        }
    } else { 
        let table_entry: *mut TableDescriptor = table_entry.cast::<TableDescriptor>();

        if unsafe { table_entry.read()  }.is_valid() {
            return Err(MappingError::LevelEntryUnset(curr_level));
        }

        let next_lvl_table_pa: usize = unsafe { *table_entry  }.get_pa() << 12;
        let next_lvl_table: *mut UnifiedTranslationTable = PAGE_ALLOCATOR.get().get_mapped_frame(PhysicalPage::new(PAddr(next_level_table_pa))).cast::<KernelTranslationTable>();

        set_table_descriptor(va, target_level, curr_level + 1, next_lvl_table)?;
    }

    return Ok(());
}

pub unsafe fn map_pa_to_va(pa: usize, va: usize, translation_table: *mut UnifiedTranslationTable, is_huge_page: bool) -> Result<(), MappingError> {
    
    let mut table_descriptor: TableDescriptor;
    loop {
        match get_translation_descriptor(va, 2, 0, translation_table) {
            Ok(translation_desc) => {
                table_descriptor = unsafe { translation_desc.table };
                break;
            },
            Err(MappingError::LevelEntryUnset(lvl)) => {
                //let translation_desc: TranslationDescriptor = get_table_descriptor_user(va, lvl, 0, translation_table_ptr).unwrap();
                let frame = PAGE_ALLOCATOR.get().alloc_mapped_frame();
                let intermediate_descriptor = TranslationDescriptor { table: TableDescriptor::new(frame.paddr), };
                //This ideally shouldn't panic as the level that failed should have a valid table
                set_table_descriptor(va, lvl, 0, translation_table, intermediate_descriptor).unwrap();
            },
            Err(e) => unreachable!(),
        }
    }

    if is_huge_page {
        
        if table_descriptor.is_valid() {
            if table_descriptor.is_table_descriptor() {
                return Err(MappingError::TableDescriptorPresent);
            } else {
                //Can swap this out for leaf table spot taken as well
                return Err(MappingError::HugePagePresent);
            }
        }

        let aligned_pa = (pa / 0x200_000) * 0x200_000;
        let new_lef = LeafDescriptor::new(aligned_pa).set_global().difference(LeafDescriptor::IS_PAGE_DESCRIPTOR);
        new_leaf.set_user_permissions(true);
        
        //This ideally shouldn't panic as the loop above fills in any missing entries
        set_translation_descriptor(va, 2, 0, translation_table, TranslationDescriptor { leaf: new_leaf }).unwrap();

        return Ok(());
    }
    
    //Need to allocate intermediate page
    if !table_descriptor.is_valid() {
         
        let frame = PAGE_ALLOCATOR.get().alloc_mapped_frame();
        let intermediate_descriptor = TranslationDescriptor { table: TableDescriptor::new(frame.paddr), };
        //This ideally shouldn't panic as the level that failed should have a valid table
        set_table_descriptor(va, lvl, 0, translation_table, intermediate_descriptor).unwrap();
    } else if !table_descriptor.is_table_descriptor() {
        //huge page present where table descriptor is expected
        return Err(MappingError::HugePagePresent);
    }
    
    let leaf_descriptor = get_translation_descriptor(va, 3, 0, translation_table).unwrap();
    let leaf_descriptor = unsafe { leaf_descriptor.leaf  };

    if !leaf_descriptor.is_valid() {
        return Err(MappingError::LeafTableSpotTaken);
    }

    let aligned_pa = (pa / PG_SZ) * PG_SZ;
    leaf_descriptor = LeafDescriptor::new(aligned_pa).set_global();
    //TODO: add this function to machine.rs
    leaf_descriptor.set_user_permissions(true);

    set_translation_descriptor(va, 3, 0, translation_table, TranslationDescriptor { leaf: leaf_descriptor }).unwrap();
    return Ok(());
}

//TODO: rewrite all of these

pub unsafe fn map_device(pa: usize) -> NonNull<()> {
    let pa_aligned = (pa / PG_SZ) * PG_SZ;
    let kernel_translation_table: *mut UnifiedTranslationTable = &raw mut KERNEL_TRANSLATION_TABLE;

    let vaddr: usize = first_unused_virt_page_addr(kernel_translation_table, 0).expect("OUt of space in the kernel page table!");
    let new_desc = LeafDescriptor::new(pa_aligned).set_mair(1).set_global();
    set_translation_descriptor(vaddr, 3, 0, kernel_translation_table, TranslationDescriptor { leaf: new_desc }).unwrap();

    //TODO: continue here
}

/*
/// not thread safe
pub unsafe fn map_device(pa: usize) -> NonNull<()> {
    let pa_aligned = (pa / PG_SZ) * PG_SZ;
    let table = &raw mut KERNEL_LEAF_TABLE;
    let table_base = table.cast::<LeafDescriptor>();

    let idx = unsafe { first_unused_virt_page(table) };
    let idx = idx.expect("Out of space in kernel page table!");

    let new_desc = LeafDescriptor::new(pa_aligned).set_mair(1).set_global();
    unsafe { table_base.add(idx).write(new_desc) };

    unsafe {
        asm! {
            "dsb ISH",
            options(readonly, nostack, preserves_flags)
        }
    }
    // TEMP: Hardcoded offsets
    let off = 0x20_0000 * 14 + idx * PG_SZ + (pa - pa_aligned);
    unsafe { virt_addr_base().byte_add(off) }
}
*/

/// not thread safe
pub unsafe fn map_device_block(pa_start: usize, size: usize) -> NonNull<()> {
    let pg_aligned_start = (pa_start / PG_SZ) * PG_SZ;
    let table = &raw mut KERNEL_LEAF_TABLE;
    let table_base = table.cast::<LeafDescriptor>();

    let idx = unsafe { first_unused_virt_page(table) };
    let idx = idx.expect("Out of space in kernel page table!");

    for (pg, pg_idx) in (pg_aligned_start..(pa_start + size))
        .step_by(PG_SZ)
        .zip(idx..)
    {
        let entry = unsafe { table_base.add(pg_idx) };
        assert!(!unsafe { entry.read() }.is_valid());
        let desc = LeafDescriptor::new(pg).set_mair(1).set_global();
        unsafe { entry.write(desc) };
    }

    unsafe {
        asm! {
            "dsb ISH",
            options(readonly, nostack, preserves_flags)
        }
    }
    // TEMP: Hardcoded offsets
    unsafe {
        virt_addr_base().byte_add(0x20_0000 * 14 + idx * PG_SZ + (pa_start - pg_aligned_start))
    }
}

/// not thread safe
pub unsafe fn map_physical(pa_start: usize, size: usize) -> NonNull<()> {
    let pg_aligned_start = (pa_start / PG_SZ) * PG_SZ;
    let table = &raw mut KERNEL_LEAF_TABLE;
    let table_base = table.cast::<LeafDescriptor>();

    let idx = unsafe { first_unused_virt_page(table) };
    let idx = idx.expect("Out of space in kernel page table!");

    for (pg, pg_idx) in (pg_aligned_start..(pa_start + size))
        .step_by(PG_SZ)
        .zip(idx..)
    {
        let entry = unsafe { table_base.add(pg_idx) };
        assert!(!unsafe { entry.read() }.is_valid());
        let desc = LeafDescriptor::new(pg).set_global();
        unsafe { entry.write(desc) };
    }

    unsafe {
        asm! {
            "dsb ISH",
            options(readonly, nostack, preserves_flags)
        }
    }
    // TEMP: Hardcoded offsets
    unsafe {
        virt_addr_base().byte_add(0x20_0000 * 14 + idx * PG_SZ + (pa_start - pg_aligned_start))
    }
}

/// not thread safe
pub unsafe fn map_physical_noncacheable(pa_start: usize, size: usize) -> NonNull<()> {
    let pg_aligned_start = (pa_start / PG_SZ) * PG_SZ;
    let table = &raw mut KERNEL_LEAF_TABLE;
    let table_base = table.cast::<LeafDescriptor>();

    let idx = unsafe { first_unused_virt_page(table) };
    let idx = idx.expect("Out of space in kernel page table!");

    for (pg, pg_idx) in (pg_aligned_start..(pa_start + size))
        .step_by(PG_SZ)
        .zip(idx..)
    {
        let entry = unsafe { table_base.add(pg_idx) };
        assert!(!unsafe { entry.read() }.is_valid());
        let desc = LeafDescriptor::new(pg).set_global().set_mair(2);
        unsafe { entry.write(desc) };
    }

    unsafe {
        asm! {
            "dsb ISH",
            options(readonly, nostack, preserves_flags)
        }
    }
    // TEMP: Hardcoded offsets
    unsafe {
        virt_addr_base().byte_add(0x20_0000 * 14 + idx * PG_SZ + (pa_start - pg_aligned_start))
    }
}

