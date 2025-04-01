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

#[derive(Debug)]
pub enum MappingError {
    HugePagePresent,
    TableDescriptorPresent,
    LeafTableSpotTaken,
}

impl Display for MappingError {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            Self::HugePagePresent => {
                write!(
                    f,
                    "Huge page present is present in desired mapping location"
                )
            }
            Self::TableDescriptorPresent => {
                write!(f, "Table descriptor present in desired mapping location")
            }
            Self::LeafTableSpotTaken => write!(
                f,
                "The spot in the leaf table that is being mapped to is already taken"
            ),
        }
    }
}

//TODO: give option of setting bits for the mapping
//Maybe add option to map huge pages here
pub unsafe fn map_pa_to_va_kernel(pa: usize, va: usize) -> Result<(), MappingError> {
    //TODO: stop using these constants
    let mut index_bits = 25 - 21; //mildly redundant
    let mut mask = (1 << index_bits) - 1;
    //level 2 table index is bits 29-21
    let mut table_index = (va >> 21) & mask;

    let table_descriptor: TableDescriptor =
        unsafe { KERNEL_TRANSLATION_TABLE.0[table_index].table };

    if !table_descriptor.is_table_descriptor() {
        //Error: Huge page instead of table descriptor
        return Err(MappingError::HugePagePresent);
    }

    let mut index_add: usize = 0;
    if table_index == 15 {
        index_add = PG_SZ / 8;
    }

    index_bits = 21 - 12;
    mask = (1 << index_bits) - 1;
    let table_index_in_page = (va >> 12) & mask;
    table_index = table_index_in_page + index_add;

    let table = &raw mut KERNEL_LEAF_TABLE;
    let table_base = table.cast::<LeafDescriptor>();

    let entry = table_base.wrapping_add(table_index);

    if unsafe { entry.read() }.is_valid() {
        //Error: spot in leaf table is taken
        return Err(MappingError::LeafTableSpotTaken);
    }

    let aligned_pa = (pa / PG_SZ) * PG_SZ;
    let new_desc = LeafDescriptor::new(aligned_pa).set_global();

    unsafe { entry.write(new_desc) };

    unsafe {
        asm! {
            "dsb ISH",
            options(readonly, nostack, preserves_flags)
        }
    }

    Ok(())
}

//TODO: add option to map huge page
//TODO: add option to pass in flags
//TODO: in the future, change this to take in a mutable reference to the user translation table
//instead of just ttrb0
pub unsafe fn map_pa_to_va_user(pa: usize, va: usize, ttbr0_pa: usize) -> Result<(), MappingError> {
    let translation_table = PAGE_ALLOCATOR
        .get()
        .get_mapped_frame(PhysicalPage::new(PAddr(ttbr0_pa)))
        .cast::<UserTranslationTable>();
    let translation_table = unsafe { &mut *translation_table };

    //TODO: stop using these constants
    let mut index_bits = 25 - 21; //mildly redundant
    let mut mask = (1 << index_bits) - 1;
    //level 2 table index is bits 29-21
    let mut table_index = (va >> 21) & mask;
    let mut table_descriptor: TableDescriptor = unsafe { translation_table.0[table_index].table };

    //Need to insert new page table
    if !table_descriptor.is_valid() {
        let frame = PAGE_ALLOCATOR.get().alloc_mapped_frame();
        translation_table.0[table_index] = TranslationDescriptor {
            table: TableDescriptor::new(frame.paddr),
        };
        //Need to update table descriptor being used so that leaf insertion can occur
        table_descriptor = unsafe { translation_table.0[table_index].table };
    } else if !table_descriptor.is_table_descriptor() {
        //Error: Huge page instead of table descriptor
        return Err(MappingError::HugePagePresent);
    }

    //Regular page case
    let lvl3_pa: usize = table_descriptor.get_pa() << 12;
    let lvl3_table_ptr = PAGE_ALLOCATOR
        .get()
        .get_mapped_frame(PhysicalPage::new(PAddr(lvl3_pa)))
        .cast::<[LeafDescriptor; PG_SZ / size_of::<LeafDescriptor>()]>();

    index_bits = 21 - 12;
    mask = (1 << index_bits) - 1;
    table_index = (va >> 12) & mask;

    let table_base = lvl3_table_ptr.cast::<LeafDescriptor>();
    let entry = table_base.wrapping_add(table_index);
    if unsafe { entry.read() }.is_valid() {
        //Error: spot in leaf table is taken
        return Err(MappingError::LeafTableSpotTaken);
    }

    let aligned_pa = (pa / PG_SZ) * PG_SZ;

    let new_desc = LeafDescriptor::new(aligned_pa).set_global();
    unsafe { entry.write(new_desc) };

    unsafe {
        asm! {
            "dsb ISH",
            options(readonly, nostack, preserves_flags)
        }
    }

    Ok(())
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
