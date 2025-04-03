use core::{
    arch::asm,
    fmt::{Display, Formatter},
    ptr::{addr_of, NonNull},
};

use crate::arch::memory::palloc::Size4KiB;
use crate::arch::memory::{KERNEL48_USER25_TCR_EL1, KERNEL48_USER48_TCR_EL1};

use super::{
    machine::{LeafDescriptor, TableDescriptor, TranslationDescriptor},
    palloc::{PhysicalPage, PAGE_ALLOCATOR},
    physical_addr,
    table::PageTablePtr,
};

const PG_SZ: usize = 0x1000;
const TRANSLATION_TABLE_SIZE: usize = PG_SZ / size_of::<TranslationDescriptor>();

/// This translation table is used for the user and kernel page tables once the 48 bit address
/// space is enabled
#[repr(C, align(4096))]
pub struct UnifiedTranslationTable(pub [TranslationDescriptor; TRANSLATION_TABLE_SIZE]);

#[unsafe(no_mangle)]
pub static mut KERNEL_UNIFIED_TRANSLATION_TABLE: UnifiedTranslationTable = UnifiedTranslationTable(
    [TranslationDescriptor {
        table: TableDescriptor::empty(),
    }; TRANSLATION_TABLE_SIZE],
);

//This is used for the 25 bit address space
const KERNEL_LEAF_TABLE_SIZE: usize = PG_SZ / 8 * 2;

#[repr(C, align(128))]
pub struct KernelTranslationTable(pub [TranslationDescriptor; 16]);

#[repr(C, align(4096))]
pub struct KernelLeafTable(pub [LeafDescriptor; KERNEL_LEAF_TABLE_SIZE]);

#[allow(dead_code)]
const USER_PG_SZ: usize = 0x1000;

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

pub fn alloc_top_page_table() -> PageTablePtr {
    let top_level_entries = 4096 / 8; // TODO

    // TODO: waste less space
    let page = PAGE_ALLOCATOR.get().alloc_mapped_frame::<Size4KiB>();
    PageTablePtr::from_partial_page(page, top_level_entries)
}

pub fn alloc_page_table() -> PageTablePtr {
    let page = PAGE_ALLOCATOR.get().alloc_mapped_frame::<Size4KiB>();
    PageTablePtr::from_full_page(page)
}

pub unsafe fn init_kernel_48bit() {
    let table: *mut UnifiedTranslationTable = &raw mut KERNEL_UNIFIED_TRANSLATION_TABLE;
    let table = PageTablePtr::from_ptr(table);
    let kernel_vmem_base = (&raw const __rpi_virt_base) as usize;

    // TEMP: 13 x 2MB = 26MB for heap
    for idx in 0..14 {
        let paddr = 0x20_0000 * idx;
        let vaddr = kernel_vmem_base + paddr;
        let leaf = LeafDescriptor::new(paddr)
            .clear_pxn()
            .set_global()
            .difference(LeafDescriptor::IS_PAGE_DESCRIPTOR);
        unsafe { set_translation_descriptor(table, vaddr, 2, 0, leaf.into(), true).unwrap() };
    }

    // TEMP: 4MB of virtual addresses (1K pages) for kernel mmaping
    let paddr = physical_addr((&raw const KERNEL_LEAF_TABLE).addr()).unwrap() as usize;
    let vaddr = kernel_vmem_base + 14 * 0x20_0000;
    let leaf = TableDescriptor::new(paddr);
    unsafe { set_translation_descriptor(table, vaddr, 2, 0, leaf.into(), true).unwrap() };

    let paddr = physical_addr((&raw const KERNEL_LEAF_TABLE).addr() + PG_SZ).unwrap() as usize;
    let vaddr = kernel_vmem_base + 15 * 0x20_0000;
    let leaf = TableDescriptor::new(paddr);
    unsafe { set_translation_descriptor(table, vaddr, 2, 0, leaf.into(), true).unwrap() };
}

/// Must be run on every core
pub unsafe fn switch_to_kernel_48bit() {
    unsafe extern "C" {
        fn switch_kernel_vmem(ttbr1_el1: usize, tcr_el1: usize);
        fn switch_user_tcr_el1(tcr_el1: usize);
    }

    let table: *mut UnifiedTranslationTable = &raw mut KERNEL_UNIFIED_TRANSLATION_TABLE;
    let table_paddr = physical_addr(table.addr()).unwrap() as usize;

    unsafe {
        switch_kernel_vmem(table_paddr, KERNEL48_USER25_TCR_EL1 as usize);
        switch_user_tcr_el1(KERNEL48_USER48_TCR_EL1 as usize);
    }
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
            Self::HugePagePresent => write!(
                f,
                "Huge page present is present in desired mapping location"
            ),
            Self::TableDescriptorPresent => {
                write!(f, "Table descriptor present in desired mapping location")
            }
            Self::LeafTableSpotTaken => write!(
                f,
                "The spot in the leaf table that is being mapped to is already taken"
            ),
            Self::LevelEntryUnset(value) => {
                write!(f, "The level {} entry for this address is invalid", value)
            }
        }
    }
}

pub unsafe fn get_translation_descriptor(
    mut table: PageTablePtr,
    va: usize,
    target_level: u8,
    mut curr_level: u8,
) -> Result<TranslationDescriptor, MappingError> {
    assert!(target_level <= 3);
    assert!(curr_level <= 3);

    loop {
        let mask = 0b111111111;
        //12 bits for 4096 byte page offset, after that its 9 bits for each level
        let table_index = (va >> (12 + (9 * (3 - curr_level)))) & mask;
        let descriptor = unsafe { table.get_entry(table_index) };

        if curr_level == target_level {
            //Now at target level and can return the entry
            return Ok(descriptor);
        }

        let intermediate_descriptor = unsafe { descriptor.table };
        if !intermediate_descriptor.is_valid() {
            return Err(MappingError::LevelEntryUnset(curr_level));
        } else if !intermediate_descriptor.is_table_descriptor() {
            return Err(MappingError::HugePagePresent);
        }

        table = PageTablePtr::from_full_page(PhysicalPage::<Size4KiB>::new(
            intermediate_descriptor.get_pa(),
        ));
        curr_level += 1;
    }
}

//Fill intermediate indicates that any missing page table levels should be created
pub unsafe fn set_translation_descriptor(
    mut table: PageTablePtr,
    va: usize,
    target_level: u8,
    mut curr_level: u8,
    descriptor: TranslationDescriptor,
    fill_intermediate: bool,
) -> Result<(), MappingError> {
    assert!(target_level <= 3);
    assert!(curr_level <= 3);

    loop {
        let mask = 0b111111111;
        //12 bits for 4096 byte page offset, after that its 9 bits for each level
        let table_index = (va >> (12 + (9 * (3 - curr_level)))) & mask;

        if curr_level == target_level {
            //Target level reached
            unsafe {
                table.set_entry(table_index, descriptor);
                asm!("dsb ISH", options(readonly, nostack, preserves_flags));
            }
            return Ok(());
        }

        let descriptor = unsafe { table.get_entry(table_index) };
        let mut intermediate_descriptor = unsafe { descriptor.table };
        if !intermediate_descriptor.is_valid() {
            if !fill_intermediate {
                return Err(MappingError::LevelEntryUnset(curr_level));
            }

            let new_table = alloc_page_table();
            intermediate_descriptor = new_table.to_descriptor();

            unsafe { table.set_entry(table_index, intermediate_descriptor.into()) };
        } else if !intermediate_descriptor.is_table_descriptor() {
            return Err(MappingError::HugePagePresent);
        }

        table = PageTablePtr::from_full_page(PhysicalPage::<Size4KiB>::new(
            intermediate_descriptor.get_pa(),
        ));

        curr_level += 1;
    }
}

pub unsafe fn map_va_to_pa(
    table: PageTablePtr,
    pa: usize,
    va: usize,
    is_huge_page: bool,
    user_permission: bool,
) -> Result<(), MappingError> {
    //Need level 2 table in both cases to check for huge page
    let table_descriptor: TableDescriptor;
    match unsafe { get_translation_descriptor(table, va, 2, 0) } {
        Ok(translation_descriptor) => table_descriptor = unsafe { translation_descriptor.table },
        Err(MappingError::LevelEntryUnset(_lvl)) => table_descriptor = TableDescriptor::empty(),
        Err(e) => return Err(e),
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

        let aligned_pa = (pa / 0x20_0000) * 0x20_0000;
        let new_leaf = LeafDescriptor::new(aligned_pa)
            .set_global()
            .difference(LeafDescriptor::IS_PAGE_DESCRIPTOR)
            .set_user_permissions(user_permission);

        //This ideally shouldn't panic as the get should have filled in the intermediate pages
        unsafe {
            set_translation_descriptor(table, va, 2, 0, new_leaf.into(), false).unwrap();
        }

        return Ok(());
    }

    //Regular page case
    if table_descriptor.is_valid() {
        if !table_descriptor.is_table_descriptor() {
            //huge page present where table descriptor is expected
            return Err(MappingError::HugePagePresent);
        }

        //NOTE: This can be slighlty optimized
        let leaf_translation_descriptor =
            unsafe { get_translation_descriptor(table, va, 3, 0).unwrap() };
        let leaf_descriptor = unsafe { leaf_translation_descriptor.leaf };

        if leaf_descriptor.is_valid() {
            return Err(MappingError::LeafTableSpotTaken);
        }
    }

    let aligned_pa = (pa / PG_SZ) * PG_SZ;
    let leaf_descriptor = LeafDescriptor::new(aligned_pa)
        .set_global()
        .set_user_permissions(user_permission);

    unsafe { set_translation_descriptor(table, va, 3, 0, leaf_descriptor.into(), true).unwrap() };
    return Ok(());
}

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
