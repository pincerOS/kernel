use crate::sync::UnsafeInit;
use alloc::boxed::Box;
use core::{
    arch::asm,
    fmt::{Display, Formatter},
    mem::MaybeUninit,
    ptr::{self, addr_of, NonNull},
    sync::atomic::{AtomicUsize, Ordering},
};

use super::{
    machine::{LeafDescriptor, TableDescriptor, TranslationDescriptor},
    physical_addr,
};

const PG_SZ: usize = 0x1000;
const KERNEL_LEAF_TABLE_SIZE: usize = PG_SZ / 8 * 2;

#[repr(C, align(128))]
pub struct KernelTranslationTable(pub [TranslationDescriptor; 16]);

#[repr(C, align(4096))]
pub struct KernelLeafTable(pub [LeafDescriptor; KERNEL_LEAF_TABLE_SIZE]);

pub const USER_PG_SZ: usize = 0x1000;
const USER_LEAF_TABLE_SIZE: usize = USER_PG_SZ / 8 * 2;

//This is public so that it can be placed in the PCB later
#[repr(C, align(128))]
pub struct UserTranslationTable(pub [TranslationDescriptor; 16]);

#[repr(C, align(4096))]
pub struct UserLeafTable(pub [LeafDescriptor; USER_LEAF_TABLE_SIZE]);

fn virt_addr_base() -> NonNull<()> {
    NonNull::new(ptr::with_exposed_provenance_mut(0xFFFF_FFFF_FE00_0000)).unwrap()
}

#[allow(improper_ctypes)]
unsafe extern "C" {
    static __rpi_phys_binary_end_addr: ();
}

//1000 pages
#[repr(C, align(4096))]
struct BigTable([u8; PG_SZ * 1000]);

//Logic for current frame allocator:
//va to pa: va - page allocator va + page allocator pa
//pa to va: pa - page allocator pa + page allocator va
//This is the logic for the current intermediate page allocator
pub struct PageAlloc {
    table_va: usize,
    table_pa: usize,
    alloc_offset: AtomicUsize,
}

impl PageAlloc {
    fn new(ptr_to_table: *const BigTable) -> PageAlloc {
        PageAlloc {
            table_va: ptr_to_table as usize,
            table_pa: physical_addr(ptr_to_table.addr()).unwrap() as usize,
            alloc_offset: AtomicUsize::new(0),
        }
    }

    /// Allocates a page of memory to be used for page tables
    /// Returns a tuple, where the first value is the virtual address of the page and the second is
    /// the physical address of the page
    pub fn alloc_frame(&self) -> (usize, usize) {
        let va: usize = self.table_va + self.alloc_offset.fetch_add(PG_SZ, Ordering::Relaxed);
        let pa: usize = va - self.table_va + self.table_pa;
        (va, pa)
    }
}

#[allow(dead_code)]
unsafe fn kernel_paddr_to_vaddr(paddr: usize) -> *mut () {
    core::ptr::with_exposed_provenance_mut(paddr + (virt_addr_base().as_ptr() as usize))
}

pub static PAGE_ALLOCATOR: UnsafeInit<PageAlloc> = unsafe { UnsafeInit::uninit() };

static PHYSICAL_ALLOC_BASE: AtomicUsize = AtomicUsize::new(0);

unsafe fn init_page_allocator() {
    let mut data_box: Box<MaybeUninit<BigTable>> = Box::new_uninit();
    unsafe { core::ptr::write_bytes(data_box.as_mut_ptr(), 0, 1) }; // zero the region
    let data_ptr: *const BigTable = Box::into_raw(data_box).cast::<BigTable>();
    unsafe { PAGE_ALLOCATOR.init(PageAlloc::new(data_ptr)) };
}

pub unsafe fn init_physical_alloc() {
    // TODO: proper physical memory layout documentation
    let base = 0x20_0000 * 16;
    PHYSICAL_ALLOC_BASE.store(base, Ordering::SeqCst);

    unsafe { init_page_allocator() };
}

pub fn create_user_table(phys_base: usize) -> alloc::boxed::Box<UserTranslationTable> {
    let mut table = alloc::boxed::Box::new(UserTranslationTable(
        [TranslationDescriptor {
            table: TableDescriptor::empty(),
        }; 16],
    ));
    let root_region_size = 0x20_0000; // 2 MiB
    for (i, desc) in table.0[1..8].iter_mut().enumerate() {
        let phys_frame = phys_base + root_region_size * i;
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

    let phys_base = PHYSICAL_ALLOC_BASE.fetch_add(region_size, Ordering::Relaxed);

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
    TableDescriptorNotValid,
    LeafTableSpotNotValid,
    MemoryRangeCollision, //used with mmap
    NotInMemoryRange,     //used with mmap
    RequestedSizeUnavailable,
}

impl Display for MappingError {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            Self::HugePagePresent => {
                write!(
                    f,
                    "Huge page present is present where a table descriptor is expected"
                )
            }
            Self::TableDescriptorPresent => {
                write!(f, "Table descriptor present in desired mapping location")
            }
            Self::LeafTableSpotTaken => write!(
                f,
                "The spot in the leaf table that is being mapped to is already taken"
            ),
            Self::MemoryRangeCollision => write!(f, "A mapped memory range collides with this one"),
            Self::RequestedSizeUnavailable => {
                write!(f, "A memory range for the requested size is unavailable")
            }
            Self::TableDescriptorNotValid => {
                write!(f, "The table descriptor for this page is not valid")
            }
            Self::LeafTableSpotNotValid => {
                write!(f, "The leaf table entry for this page is not valid")
            }
            Self::NotInMemoryRange => write!(f, "The address in not in a mmapped memory range"),
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
pub unsafe fn map_pa_to_va_user(
    pa: usize,
    va: usize,
    translation_table: &mut Box<UserTranslationTable>,
) -> Result<(), MappingError> {
    //TODO: stop using these constants
    let mut index_bits = 25 - 21; //mildly redundant
    let mut mask = (1 << index_bits) - 1;
    //level 2 table index is bits 29-21
    let mut table_index = (va >> 21) & mask;
    let mut table_descriptor: TableDescriptor = unsafe { translation_table.0[table_index].table };

    //Need to insert new page table
    if !table_descriptor.is_valid() {
        let (_pt_va, pt_pa) = PAGE_ALLOCATOR.get().alloc_frame();
        translation_table.0[table_index] = TranslationDescriptor {
            table: TableDescriptor::new(pt_pa),
        };
        //Need to update table descriptor being used so that leaf insertion can occur
        table_descriptor = unsafe { translation_table.0[table_index].table };
    } else if !table_descriptor.is_table_descriptor() {
        //Error: Huge page instead of table descriptor
        return Err(MappingError::HugePagePresent);
    }

    //Regular page case
    let lvl3_pa: usize = table_descriptor.get_pa() << 12;
    let lvl3_va: usize = lvl3_pa - PAGE_ALLOCATOR.get().table_pa + PAGE_ALLOCATOR.get().table_va;
    let lvl3_table_ptr: *mut [LeafDescriptor; PG_SZ] = lvl3_va as *mut [LeafDescriptor; PG_SZ];

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

    let mut new_desc = LeafDescriptor::new(aligned_pa).set_global();
    new_desc.set_user_permissions(true);
    unsafe { entry.write(new_desc) };

    unsafe {
        asm! {
            "dsb ISH",
            options(readonly, nostack, preserves_flags)
        }
    }

    Ok(())
}

//This function unmaps a page from the virtual address space and returns its virtual addr so that
//the page can be freed
//TODO: add in a mechanism for freeing page tables
//TODO: account for huge page case
pub unsafe fn unmap_va_user(
    va: usize,
    translation_table: &mut Box<UserTranslationTable>,
) -> Result<usize, MappingError> {
    //TODO: stop using these constants
    let mut index_bits = 25 - 21; //mildly redundant
    let mut mask = (1 << index_bits) - 1;
    //level 2 table index is bits 29-21
    let mut table_index = (va >> 21) & mask;
    let table_descriptor: TableDescriptor = unsafe { translation_table.0[table_index].table };

    if !table_descriptor.is_valid() {
        //Error: the table descriptor for this page is not valid
        return Err(MappingError::TableDescriptorNotValid);
    } else if !table_descriptor.is_table_descriptor() {
        //Error: Huge page instead of table descriptor
        return Err(MappingError::HugePagePresent);
    }

    //Regular page case
    let lvl3_pa: usize = table_descriptor.get_pa() << 12;
    let lvl3_va: usize = lvl3_pa - PAGE_ALLOCATOR.get().table_pa + PAGE_ALLOCATOR.get().table_va;
    let lvl3_table_ptr: *mut [LeafDescriptor; PG_SZ] = lvl3_va as *mut [LeafDescriptor; PG_SZ];

    index_bits = 21 - 12;
    mask = (1 << index_bits) - 1;
    table_index = (va >> 12) & mask;

    let table_base: *const LeafDescriptor = lvl3_table_ptr.cast::<LeafDescriptor>();
    let entry: *mut LeafDescriptor = table_base.wrapping_add(table_index) as *mut LeafDescriptor;

    if !(unsafe { entry.read() }.is_valid()) {
        //Error: leaf table entry for this page is page is not present
        return Err(MappingError::LeafTableSpotNotValid);
    }

    let page_pa: usize = unsafe { entry.read() }.get_pa() << 12;
    unsafe {
        (*entry).set_valid(false);
    }

    //Invalidate the TLB entry for this address
    //TODO: double check that these are the best instructions to use for this
    unsafe {
        asm! {
            "dsb ISH",
            "tlbi vaae1, {0}",
            in(reg) (va >> 12)
            //options(readonly, nostack, preserves_flags)
        }
    }

    //It will be up to the caller to free this page
    return Ok(page_pa);
}

//This will probably assume that the user has removed all pages which cannot be returned to the
//page allocator
pub unsafe fn clear_user_vaddr_space(translation_table: &mut Box<UserTranslationTable>) {
    //Index bits but -1 to get the number of entries at this level
    let num_lvl2_entries = 2 << ((25 - 21) - 1);

    for lvl2_index in 0..num_lvl2_entries {
        let table_descriptor: TableDescriptor = unsafe { translation_table.0[lvl2_index].table };

        if !table_descriptor.is_valid() {
            continue;
        } else if !table_descriptor.is_table_descriptor() {
            //TODO: deallocate huge page here
            continue;
        }

        let lvl3_pa: usize = table_descriptor.get_pa() << 12;
        let lvl3_va: usize =
            lvl3_pa - PAGE_ALLOCATOR.get().table_pa + PAGE_ALLOCATOR.get().table_va;
        let lvl3_table_ptr: *mut [LeafDescriptor; PG_SZ] = lvl3_va as *mut [LeafDescriptor; PG_SZ];

        let num_lvl3_entries = PG_SZ / size_of::<LeafDescriptor>();
        let table_base: *const LeafDescriptor = lvl3_table_ptr.cast::<LeafDescriptor>();

        for lvl3_index in 0..num_lvl3_entries {
            let entry: *const LeafDescriptor = table_base.wrapping_add(lvl3_index);

            if !(unsafe { entry.read() }.is_valid()) {
                continue;
            }

            let _page_pa: usize = unsafe { entry.read() }.get_pa() << 12;
            //TODO: free the page here
        }

        //TODO: deallocate the intermediate page

        //No need to set the spot as invalid as this page is being deallocated
    }
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
