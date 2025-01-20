use core::{
    arch::asm,
    ptr::{self, addr_of, NonNull},
};

use super::{
    machine::{LeafDescriptor, TableDescriptor, TranslationDescriptor},
    physical_addr,
};

#[repr(C, align(256))]
struct KernelTranslationTable([TranslationDescriptor; 32]);

const PG_SZ: usize = 0x1000;

#[repr(C, align(4096))]
struct KernelLeafTable([LeafDescriptor; PG_SZ / 8]);

fn virt_addr_base() -> NonNull<()> {
    NonNull::new(ptr::with_exposed_provenance_mut(0xFFFF_FFFF_FE00_0000)).unwrap()
}

/// only call once!
pub unsafe fn init() {
    unsafe {
        KERNEL_TRANSLATION_TABLE.0[1] = TranslationDescriptor {
            table: TableDescriptor::new(
                physical_addr(addr_of!(KERNEL_LEAF_TABLE).addr()).unwrap() as usize
            ),
        };
    }
}

/// not thread safe
pub unsafe fn map_device(pa: usize) -> NonNull<()> {
    let pa_aligned = (pa / PG_SZ) * PG_SZ;
    let table = unsafe { &mut KERNEL_LEAF_TABLE };
    let (idx, entry) = table
        .0
        .iter_mut()
        .enumerate()
        .find(|(_, desc)| !desc.is_valid())
        .unwrap();
    *entry = LeafDescriptor::new(pa_aligned).set_mair(1).set_global();
    unsafe {
        asm! {
            "dsb ISH",
            options(readonly, nostack, preserves_flags)
        }
    }
    unsafe { virt_addr_base().byte_add(0x20_0000 + idx * PG_SZ + (pa - pa_aligned)) }
}

/// not thread safe
pub unsafe fn map_physical(pa_start: usize, size: usize) -> NonNull<()> {
    let pg_aligned_start = (pa_start / PG_SZ) * PG_SZ;
    println!("a {pg_aligned_start:X} st {pa_start:X} sz {size:X} ");
    let table = unsafe { &mut KERNEL_LEAF_TABLE };
    let idx = table
        .0
        .iter_mut()
        .position(|desc| !desc.is_valid())
        .unwrap();
    for (pg, pg_idx) in (pg_aligned_start..(pa_start + size))
        .step_by(PG_SZ)
        .zip(idx..)
    {
        table.0[pg_idx] = LeafDescriptor::new(pg).set_global();
    }
    unsafe {
        asm! {
            "dsb ISH",
            options(readonly, nostack, preserves_flags)
        }
    }
    unsafe { virt_addr_base().byte_add(0x20_0000 + idx * PG_SZ + (pa_start - pg_aligned_start)) }
}

#[no_mangle]
static mut KERNEL_LEAF_TABLE: KernelLeafTable =
    KernelLeafTable([LeafDescriptor::empty(); PG_SZ / 8]);

#[no_mangle]
static mut KERNEL_TRANSLATION_TABLE: KernelTranslationTable = KernelTranslationTable(
    [TranslationDescriptor {
        table: TableDescriptor::empty(),
    }; 32],
);
