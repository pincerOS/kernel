use alloc::boxed::Box;
use core::{
    arch::asm,
    fmt::{Display, Formatter},
    ptr::{addr_of, NonNull},
};

use crate::arch::memory::{KERNEL48_USER25_TCR_EL1, KERNEL48_USER48_TCR_EL1};

use super::{
    machine::{LeafDescriptor, TableDescriptor, TranslationDescriptor},
    palloc::{PAddr, PhysicalPage, PAGE_ALLOCATOR},
    physical_addr,
};

const PG_SZ: usize = 0x1000;
const TRANSLATION_TABLE_SIZE: usize = PG_SZ / size_of::<TranslationDescriptor>();

/// This translation table is used for the user and kernel page tables once the 48 bit address
/// space is enabled
#[repr(C, align(4096))]
pub struct UnifiedTranslationTable(pub [TranslationDescriptor; TRANSLATION_TABLE_SIZE]);

#[unsafe(no_mangle)]
static mut KERNEL_UNIFIED_TRANSLATION_TABLE: UnifiedTranslationTable = UnifiedTranslationTable(
    [TranslationDescriptor {
        table: TableDescriptor::empty(),
    }; TRANSLATION_TABLE_SIZE],
);

//These are used for the 25 bit address space
const KERNEL_LEAF_TABLE_SIZE: usize = PG_SZ / 8 * 2;

#[repr(C, align(128))]
pub struct KernelTranslationTable(pub [TranslationDescriptor; 16]);

#[repr(C, align(4096))]
pub struct KernelLeafTable(pub [LeafDescriptor; KERNEL_LEAF_TABLE_SIZE]);

const USER_PG_SZ: usize = 0x1000;
const USER_LEAF_TABLE_SIZE: usize = USER_PG_SZ / 8 * 2;

//This is public so that it can be placed in the PCB later
#[repr(C, align(128))]
pub struct UserTranslationTable(pub [TranslationDescriptor; 16]);

#[repr(C, align(4096))]
pub struct UserLeafTable(pub [LeafDescriptor; USER_LEAF_TABLE_SIZE]);
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

//Used with 48 bit vaddr space
pub fn create_user_table_v2(phys_base: PAddr) -> alloc::boxed::Box<UnifiedTranslationTable> {
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
        //TODO: fix this to use new set fill methods
        let translation_descriptor = TranslationDescriptor{ leaf: leaf };
        //map_pa_to_va(phys_frame, virt_region_base + root_region_size * i, table_ptr, true).unwrap();
        //This will fill in the intermediate pages on the way
        unsafe { set_translation_descriptor(virt_region_base + root_region_size * i, 2, 0, table_ptr, translation_descriptor, true).unwrap(); }
    }

    return table;
}

//Used with 48 bit vaddr space
pub unsafe fn create_user_region_v2() -> (*mut [u8], Box<UnifiedTranslationTable>) {
    let virt_region_base = 0x20_0000;
    let region_size = 0x20_0000 * 7;

    let (phys_base, _) = PAGE_ALLOCATOR.get().alloc_range(region_size, 0x20_0000);

    let user_table = create_user_table_v2(phys_base);
    let user_table_vaddr = (&*user_table as *const UnifiedTranslationTable).addr();
    let user_table_phys = physical_addr(user_table_vaddr).unwrap();
    println!("creating user table, {:#010x}", user_table_phys);

    let ptr = core::ptr::with_exposed_provenance_mut::<u8>(virt_region_base);
    let slice = core::ptr::slice_from_raw_parts_mut(ptr, region_size);
    (slice, user_table)
}

// Used with 48 bit vaddr space
/// only call once!
pub unsafe fn init_unified() {

    let kernel_translation_table: *mut UnifiedTranslationTable = &raw mut KERNEL_UNIFIED_TRANSLATION_TABLE;
    unsafe {
        
        for i in 1..14 {
            //let translation_descriptor = TranslationDescriptor { leaf: LeafDescriptor::new(0x20_0000 * i).set_global().difference(LeafDescriptor::IS_PAGE_DESCRIPTOR)  };
            //set_translation_descriptor(0x20_0000 * i, 2, 0, );
            map_pa_to_va(0x20_0000 * i, 0x20_0000, kernel_translation_table, true).unwrap();
        }
        
        //No leaf table
    }
}

pub unsafe fn init_kernel_48bit() {
    let table: *mut UnifiedTranslationTable = &raw mut KERNEL_UNIFIED_TRANSLATION_TABLE;
    let kernel_vmem_base = (&raw const __rpi_virt_base) as usize;

    // TEMP: 13 x 2MB = 26MB for heap
    for idx in 0..14 {
        let paddr = 0x20_0000 * idx;
        let vaddr = kernel_vmem_base + paddr;
        let leaf = LeafDescriptor::new(paddr)
            .clear_pxn()
            .set_global()
            .difference(LeafDescriptor::IS_PAGE_DESCRIPTOR);
        unsafe { set_translation_descriptor(vaddr, 2, 0, table, leaf.into(), true).unwrap() };
    }

    // TEMP: 4MB of virtual addresses (1K pages) for kernel mmaping
    let paddr = physical_addr((&raw const KERNEL_LEAF_TABLE).addr()).unwrap() as usize;
    let vaddr = kernel_vmem_base + 14 * 0x20_0000;
    let leaf = TableDescriptor::new(paddr);
    unsafe { set_translation_descriptor(vaddr, 2, 0, table, leaf.into(), true).unwrap() };

    let paddr = physical_addr((&raw const KERNEL_LEAF_TABLE).addr() + PG_SZ).unwrap() as usize;
    let vaddr = kernel_vmem_base + 15 * 0x20_0000;
    let leaf = TableDescriptor::new(paddr);
    unsafe { set_translation_descriptor(vaddr, 2, 0, table, leaf.into(), true).unwrap() };
}

/// Must be run on every core
pub unsafe fn switch_to_kernel_48bit() {
    unsafe extern "C" {
        fn switch_kernel_vmem(ttbr1_el1: usize, tcr_el1: usize);
        fn switch_user_tcr_el1(tcr_el1: usize);
    }

    let table: *mut UnifiedTranslationTable = &raw mut KERNEL_UNIFIED_TRANSLATION_TABLE;
    let table_paddr = physical_addr(table.addr()).unwrap() as usize;

    unsafe { switch_kernel_vmem(table_paddr, KERNEL48_USER25_TCR_EL1 as usize) };
    // TODO: enable once user table construction is updated
    if false {
        unsafe { switch_user_tcr_el1(KERNEL48_USER48_TCR_EL1 as usize) };
    }
}

//Used with the 48 bit address space
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
        let mov_amt: usize = PG_SZ * TRANSLATION_TABLE_SIZE.pow((3 - curr_level) as u32);
        let table_base = table.cast::<TranslationDescriptor>();
        for idx in 0..TRANSLATION_TABLE_SIZE {
            let entry = table_base.wrapping_add(idx);
            let mut descriptor: TableDescriptor = unsafe { entry.read().table };

            if !descriptor.is_valid() {
                let frame = PAGE_ALLOCATOR.get().alloc_mapped_frame();
                descriptor = TableDescriptor::new(frame.paddr);
                let intermediate_descriptor = TranslationDescriptor { table: descriptor, };
                unsafe { entry.write(intermediate_descriptor) };
            }
            
            let next_table: *mut UnifiedTranslationTable = (descriptor.get_pa() << 12) as *mut UnifiedTranslationTable;
            if let Some(addr) = unsafe { first_unused_virt_page_addr(next_table, curr_level + 1) } {
                return Some((idx * mov_amt) + addr);
            }

        }
    }

    return None;
}

pub fn create_user_table(phys_base: PAddr) -> alloc::boxed::Box<UserTranslationTable> {
    let mut table = alloc::boxed::Box::new(UserTranslationTable(
        [TranslationDescriptor {
            table: TableDescriptor::empty(),
        }; 16],
    ));
    let root_region_size = 0x20_0000; // 2 MiB
    for (i, desc) in table.0[1..8].iter_mut().enumerate() {
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
        desc.leaf = leaf;
    }
    table
}

pub unsafe fn create_user_region() -> (*mut [u8], Box<UserTranslationTable>) {
    let virt_region_base = 0x20_0000;
    let region_size = 0x20_0000 * 7;

    let (phys_base, _) = PAGE_ALLOCATOR.get().alloc_range(region_size, 0x20_0000);

    let user_table = create_user_table(phys_base);
    let user_table_vaddr = (&*user_table as *const UserTranslationTable).addr();
    let user_table_phys = physical_addr(user_table_vaddr).unwrap();
    println!("creating user table, {:#010x}", user_table_phys);

    let ptr = core::ptr::with_exposed_provenance_mut::<u8>(virt_region_base);
    let slice = core::ptr::slice_from_raw_parts_mut(ptr, region_size);
    (slice, user_table)
}

/// only call once!
pub unsafe fn init() {
    unsafe {
        // TEMP: 13 x 2MB = 26MB for heap
        for idx in 1..14 {
            KERNEL_TRANSLATION_TABLE.0[idx] = TranslationDescriptor {
                leaf: LeafDescriptor::new(0x20_0000 * idx)
                    .set_global()
                    .difference(LeafDescriptor::IS_PAGE_DESCRIPTOR),
            };
        }
        // TEMP: 4MB of virtual addresses (1K pages) for kernel mmaping
        KERNEL_TRANSLATION_TABLE.0[14] = TranslationDescriptor {
            table: TableDescriptor::new(
                physical_addr(addr_of!(KERNEL_LEAF_TABLE).addr()).unwrap() as usize
            ),
        };
        KERNEL_TRANSLATION_TABLE.0[15] = TranslationDescriptor {
            table: TableDescriptor::new(
                physical_addr(addr_of!(KERNEL_LEAF_TABLE).addr() + PG_SZ).unwrap() as usize,
            ),
        };
    }
}

unsafe fn first_unused_virt_page(table: *mut KernelLeafTable) -> Option<usize> {
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

pub unsafe fn get_translation_descriptor(va: usize, target_level: u8, mut curr_level: u8, mut translation_table: *mut UnifiedTranslationTable) -> Result<TranslationDescriptor, MappingError> {
    assert!(target_level <= 3);
    assert!(curr_level <= 3);
    //12 bits for 4096 byte page offset, after that its 9 bits for each level
    let mask = 0b111111111;
    
    let mut table_index = (va >> (12 + (9 * (3 - curr_level)))) & mask;
    let mut translation_descriptor: *mut TranslationDescriptor = translation_table.cast::<TranslationDescriptor>().wrapping_add(table_index);
    
    //NOTE: consider swaping to for loop and always starting at 0
    while curr_level < target_level {
        
        let table_entry: *mut TableDescriptor = translation_descriptor.cast::<TableDescriptor>();
        let descriptor: TableDescriptor = unsafe { table_entry.read() };
        if !descriptor.is_valid() {
            return Err(MappingError::LevelEntryUnset(curr_level));
        }

        let next_lvl_table_pa: usize = descriptor.get_pa() << 12;
        translation_table = PAGE_ALLOCATOR.get().get_mapped_frame(PhysicalPage::new(PAddr(next_lvl_table_pa))).cast::<UnifiedTranslationTable>();

        curr_level += 1;
        table_index = (va >> (12 + (9 * (3 - curr_level)))) & mask;
        translation_descriptor = translation_table.cast::<TranslationDescriptor>().wrapping_add(table_index);
    }

    //Now at target level and can return the entry
    return Ok(unsafe { translation_descriptor.read() });    
}

//Fill intermediate indicates that any missing page table levels should be created
pub unsafe fn set_translation_descriptor(va: usize, target_level: u8, mut curr_level: u8, mut translation_table: *mut UnifiedTranslationTable, descriptor: TranslationDescriptor, fill_intermediate: bool) -> Result<(), MappingError> {
    assert!(target_level <= 3);
    assert!(curr_level <= 3);
    let mask = 0b111111111;
    let mut table_index = (va >> (12 + (9 * (3 - curr_level)))) & mask;
    let mut translation_descriptor: *mut TranslationDescriptor = translation_table.cast::<TranslationDescriptor>().wrapping_add(table_index);
    
    //NOTE: consider swaping to for loop and always starting at 0
    while curr_level < target_level {

        let table_entry: *mut TableDescriptor = translation_descriptor.cast::<TableDescriptor>();
        let mut intermediate_descriptor: TableDescriptor = unsafe { table_entry.read() };
        if !intermediate_descriptor.is_valid() {

            if !fill_intermediate {
                return Err(MappingError::LevelEntryUnset(curr_level));
            }

            let frame = PAGE_ALLOCATOR.get().alloc_mapped_frame();
            intermediate_descriptor = TableDescriptor::new(frame.paddr);
            unsafe { table_entry.write(intermediate_descriptor) };

        }

        let next_lvl_table_pa: usize = intermediate_descriptor.get_pa() << 12;
        translation_table = PAGE_ALLOCATOR.get().get_mapped_frame(PhysicalPage::new(PAddr(next_lvl_table_pa))).cast::<UnifiedTranslationTable>();

        curr_level += 1;
        table_index = (va >> (12 + (9 * (3 - curr_level)))) & mask;
        translation_descriptor = translation_table.cast::<TranslationDescriptor>().wrapping_add(table_index);
    }

    //Target level reached
    unsafe { 
        translation_descriptor.write(descriptor);  
        asm!("dsb ISH", options(readonly, nostack, preserves_flags));
    }

    return Ok(());
}

//TODO: take another look at this method and see if it can be improved
pub unsafe fn map_pa_to_va(pa: usize, va: usize, translation_table: *mut UnifiedTranslationTable, is_huge_page: bool) -> Result<(), MappingError> {
    
    let table_descriptor: TableDescriptor;
    let mut iter_count = 0;
    loop {
        assert!(iter_count < 5);
        match unsafe { get_translation_descriptor(va, 2, 0, translation_table) } {
            Ok(translation_desc) => {
                table_descriptor = unsafe { translation_desc.table };
                break;
            },
            Err(MappingError::LevelEntryUnset(lvl)) => {
                //let translation_desc: TranslationDescriptor = get_table_descriptor_user(va, lvl, 0, translation_table_ptr).unwrap();
                let frame = PAGE_ALLOCATOR.get().alloc_mapped_frame();
                let intermediate_descriptor = TranslationDescriptor { table: TableDescriptor::new(frame.paddr), };
                //This ideally shouldn't panic as the level that failed should have a valid table
                //TODO: take another look at this
                unsafe { set_translation_descriptor(va, lvl, 0, translation_table, intermediate_descriptor, false).unwrap(); }
            },
            Err(_e) => unreachable!(),
        }

        iter_count += 1;
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
        let mut new_leaf = LeafDescriptor::new(aligned_pa).set_global().difference(LeafDescriptor::IS_PAGE_DESCRIPTOR);
        new_leaf.set_user_permissions(true);
        
        //This ideally shouldn't panic as the loop above fills in any missing entries
        unsafe { set_translation_descriptor(va, 2, 0, translation_table, TranslationDescriptor { leaf: new_leaf }, false).unwrap(); }

        return Ok(());
    }
    
    //Need to allocate intermediate page
    if !table_descriptor.is_valid() {
         
        let frame = PAGE_ALLOCATOR.get().alloc_mapped_frame();
        let intermediate_descriptor = TranslationDescriptor { table: TableDescriptor::new(frame.paddr), };
        //This ideally shouldn't panic as the level that failed should have a valid table
        //TODO: take another look at this
        unsafe { set_translation_descriptor(va, 2, 0, translation_table, intermediate_descriptor, false).unwrap(); }
    } else if !table_descriptor.is_table_descriptor() {
        //huge page present where table descriptor is expected
        return Err(MappingError::HugePagePresent);
    }
    
    let leaf_translation_descriptor = unsafe { get_translation_descriptor(va, 3, 0, translation_table).unwrap() };
    let mut leaf_descriptor = unsafe { leaf_translation_descriptor.leaf };

    if !leaf_descriptor.is_valid() {
        return Err(MappingError::LeafTableSpotTaken);
    }

    let aligned_pa = (pa / PG_SZ) * PG_SZ;
    leaf_descriptor = LeafDescriptor::new(aligned_pa).set_global();
    //TODO: add this function to machine.rs
    leaf_descriptor.set_user_permissions(true);

    unsafe { set_translation_descriptor(va, 3, 0, translation_table, TranslationDescriptor { leaf: leaf_descriptor }, false).unwrap(); }
    return Ok(());
}

//Used with 48 bit vaddr space
pub unsafe fn map_physical_gen(pa_start: usize, req_size: usize, mair: u8, translation_table: *mut UnifiedTranslationTable) -> NonNull<()> {

    let pa_aligned_start = (pa_start / PG_SZ) * PG_SZ;
    let mut size = (req_size / PG_SZ) * PG_SZ;
    if (req_size % PG_SZ) != 0 {
        size += PG_SZ;
    }

    let start_vaddr: usize = unsafe { first_unused_virt_page_addr(translation_table, 0).expect("Out of space in the kernel page table!") };
    //This will break if there isn't enough space but should be a good enough temporary solution
    //until mmap is integrated

    for offset in (0..size).step_by(PG_SZ) {
        let pa: usize = pa_aligned_start + offset;
        let va: usize = start_vaddr + offset;
        let new_desc = LeafDescriptor::new(pa).set_mair(mair).set_global();
        //TODO: double check page fill selection here
        unsafe { set_translation_descriptor(va, 3, 0, translation_table, TranslationDescriptor { leaf: new_desc }, true).unwrap(); }
    }

    return NonNull::new(start_vaddr as *mut _).unwrap();
}


//TODO: give these better names and/or replace them with mmap

//Used with the 48 bit address space
pub unsafe fn map_device_v2(pa: usize) -> NonNull<()> {
    unsafe {
        let kernel_translation_table: *mut UnifiedTranslationTable = &raw mut KERNEL_UNIFIED_TRANSLATION_TABLE;
        return map_physical_gen(pa, PG_SZ, 1, kernel_translation_table);
    }
}

pub unsafe fn map_device_block_v2(pa_start: usize, req_size: usize) -> NonNull<()> {
    unsafe {
        let kernel_translation_table: *mut UnifiedTranslationTable = &raw mut KERNEL_UNIFIED_TRANSLATION_TABLE;
        return map_physical_gen(pa_start, req_size, 1, kernel_translation_table);
    }
}

pub unsafe fn map_physical_v2(pa_start: usize, req_size: usize) -> NonNull<()> {
    unsafe {
        let kernel_translation_table: *mut UnifiedTranslationTable = &raw mut KERNEL_UNIFIED_TRANSLATION_TABLE;
        return map_physical_gen(pa_start, req_size, 0, kernel_translation_table);
    }
}

pub unsafe fn map_physical_noncacheable_v2(pa_start: usize, req_size: usize) -> NonNull<()> {
    unsafe {
        let kernel_translation_table: *mut UnifiedTranslationTable = &raw mut KERNEL_UNIFIED_TRANSLATION_TABLE;
        return map_physical_gen(pa_start, req_size, 2, kernel_translation_table);
    }
}

//Function from 25 bit address space

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

//This is two adjacent pages all filled with leaf descriptors
#[unsafe(no_mangle)]
static mut KERNEL_LEAF_TABLE: KernelLeafTable =
    KernelLeafTable([LeafDescriptor::empty(); PG_SZ / 8 * 2]);

#[unsafe(no_mangle)]
static mut KERNEL_TRANSLATION_TABLE: KernelTranslationTable = KernelTranslationTable(
    [TranslationDescriptor {
        table: TableDescriptor::empty(),
    }; 16],
);
