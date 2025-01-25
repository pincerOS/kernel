#![allow(dead_code)]

use core::arch::asm;

use bitflags::bitflags;

bitflags! {
    // TODO: Address feature-dependent bits after TBI1
    #[derive(Clone, Copy, Debug)]
    pub struct TcrEl1: u64 {
        /// The size offset of the memory region addressed by TTBR0_EL1. The region size is 2(64-T0SZ) bytes.
        ///
        /// The maximum and minimum possible values for T0SZ depend on the level of translation table and the memory translation granule size.
        const T0SZ = 0b111111;
        /// Translation table walk disable for translations using TTBR0_EL1.
        ///
        /// This bit controls whether a translation table walk is performed on a TLB miss, for an address that is translated using TTBR0_EL1.
        ///
        /// Always 0b0 to allow translation table walks
        const EPD0 = 0b1 << 7;
        /// Inner cacheability attribute for memory associated with translation table walks using TTBR0_EL1.
        ///
        /// Always 0b01 as all normal memory is Write-Back Read-Allocate Write-Allocate Cacheable.
        const IRGN0 = 0b11 << 8;
        /// Outer cacheability attribute for memory associated with translation table walks using TTBR0_EL1.
        ///
        /// Always 0b01 as all normal memory is Write-Back Read-Allocate Write-Allocate Cacheable.
        const ORGN0 = 0b11 << 10;
        /// Shareability attribute for memory associated with translation table walks using TTBR0_EL1.
        ///
        /// Always 0b10 as all normal memory is Outer Shareable
        const SH0 = 0b11 << 12;
        /// Granule size for the TTBR0_EL1.
        ///
        /// 0b00 4KB
        /// 0b01 64KB
        /// 0b10 16KB
        const TG0 = 0b11 << 14;
        /// The size offset of the memory region addressed by TTBR1_EL1. The region size is 2(64-T0SZ) bytes.
        ///
        /// The maximum and minimum possible values for T0SZ depend on the level of translation table and the memory translation granule size.
        const T1SZ = 0b111111 << 16;
        /// Selects whether TTBR0_EL1 or TTBR1_EL1 defines the ASID.
        ///
        /// Always 0b0 as we always use TTBR0_EL1 to define the ASID
        const A1 = 0b1 << 22;
        /// Translation table walk disable for translations using TTBR1_EL1.
        ///
        /// This bit controls whether a translation table walk is performed on a TLB miss, for an address that is translated using TTBR1_EL1
        ///
        /// Always 0b0 to allow translation table walks
        const EPD1 = 0b1 << 23;
        /// Inner cacheability attribute for memory associated with translation table walks using TTBR1_EL1.
        ///
        /// Always 0b01 as all normal memory is Write-Back Read-Allocate Write-Allocate Cacheable.
        const IRGN1 = 0b11 << 24;
        /// Outer cacheability attribute for memory associated with translation table walks using TTBR1_EL1.
        ///
        /// Always 0b01 as all normal memory is Write-Back Read-Allocate Write-Allocate Cacheable.
        const ORGN1 = 0b11 << 26;
        /// Shareability attribute for memory associated with translation table walks using TTBR1_EL1.
        ///
        /// Always 0b10 as all normal memory is Outer Shareable
        const SH1 = 0b11 << 28;
        /// Granule size for the TTBR0_EL1.
        ///
        /// 0b01 16KB.
        /// 0b10 4KB.
        /// 0b11 64KB.
        const TG1 = 0b11 << 30;
        /// Intermediate Physical Address Size.
        ///
        /// 0b000 32 bits, 4GB.
        /// 0b001 36 bits, 64GB.
        /// 0b010 40 bits, 1TB.
        /// 0b011 42 bits, 4TB.
        /// 0b100 44 bits, 16TB.
        /// 0b101 48 bits, 256TB.
        /// 0b110 52 bits, 4PB.
        const IPS = 0b111 << 32;

        /// ASID Size.
        /// 0b0 8 bit - the upper 8 bits of TTBR0_EL1 and TTBR1_EL1 are ignored by hardware for every purpose except reading back the register, and are treated as if they are all zeros for when used for allocation and matching entries in the TLB.
        /// 0b1 16 bit - the upper 16 bits of TTBR0_EL1 and TTBR1_EL1 are used for allocation and matching in the TLB.
        const AS = 0b1 << 36;

        /// Top Byte ignored. Indicates whether the top byte of an address is used for address match for the TTBR0_EL1 region, or ignored and used for tagged addresses.
        /// 0b0 Top Byte used in the address calculation.
        /// 0b1 Top Byte ignored in the address calculation.
        const TBI0 = 0b1 << 37;

        /// Top Byte ignored. Indicates whether the top byte of an address is used for address match for the TTBR1_EL1 region, or ignored and used for tagged addresses.
        /// 0b0 Top Byte used in the address calculation.
        /// 0b1 Top Byte ignored in the address calculation.
        const TBI1 = 0b1 << 37;
    }

    // TODO: Address feature-dependent CnP bit
    #[derive(Clone, Copy, Debug)]
    pub struct TtbrEl1: u64 {
        /// Translation table base address:
        /// * Bits A[47:x] of the stage 1 translation table base address bits are in register bits[47:x].
        /// * Bits A[(x-1):0] of the stage 1 translation table base address are zero.
        ///
        /// Address bit x is the minimum address bit required to align the translation table to the size of the table. The AArch64 Virtual Memory System Architecture chapter describes how x is calculated based on the value of TCR_EL1.T0SZ, the translation stage, and the translation granule size.
        const BADDR = ((1 << 47) - 1) << 1;

        /// An ASID for the translation table base address. The TCR_EL1.A1 field selects either TTBR0_EL1.ASID or TTBR1_EL1.ASID.
        ///
        /// If the implementation has only 8 bits of ASID, then the upper 8 bits of this field are RES0.
        ///
        /// By default we select TTBR0_EL1 as the ASID
        const ASID = ((1 << 16) - 1) << 48;
    }

    // TODO: Double-check nT bit
    /// Block and page descriptors; the ends of a translation table walk
    #[derive(Clone, Copy, Debug)]
    pub struct LeafDescriptor: u64 {
        /// Valid descriptor.
        const VALID = 0b1;
        /// Whether the descriptor is for a block (huge page) or page (4KB/16KB/64KB)
        const IS_PAGE_DESCRIPTOR = 0b1 << 1;
        // TODO
        const ATTR_IDX = 0b111 << 2;
        // Used if the access is from Secure state, from Realm state using the EL2 or EL2&0 translation regimes, or from Root state.
        const NS = 0b1 << 5;
        const UNPRIVILEGED_ACCESS = 0b1 << 6;
        const READ_ONLY = 0b1 << 7;
        const SHAREABILITY = 0b11 << 8;
        const ACCESSED = 0b1 << 10;
        const NOT_GLOBAL = 0b1 << 11;
        const OA = ((1 << 36) - 1) << 12;
        const GP = 0b1 << 50;
        const DBM = 0b1 << 51;
        const CONTIGUOUS = 0b1 << 52;
        const PXN = 0b1 << 53;
        const UXN = 0b1 << 54;
    }

    // TODO: Fill this in when multilevel translation is used
    #[derive(Clone, Copy, Debug)]
    pub struct TableDescriptor: u64 {
        /// Valid descriptor.
        const VALID = 0b1;
        /// Always 1
        const IS_TABLE_DESCRIPTOR = 0b1 << 1;
        const NEXT_ADDR = ((1 << 36) - 1) << 12;
    }

    pub struct ParEl1Success: u64 {
        /// Indicates whether the instruction performed a successful address translation. 0 for the success case
        const FAULT = 0b1;
        /// Shareability attribute, for the returned output address.
        ///
        /// * 0b00 Non-shareable.
        /// * 0b10 Outer Shareable (or Device memory, or Normal memory with both Inner Non-cacheable and Outer Non-cacheable attributes).
        /// * 0b11 Inner Shareable.
        ///
        /// The value returned in this field can be the resulting attribute, as determined by any permitted
        /// implementation choices and any applicable configuration bits, instead of the value that appears in
        /// the Translation table descriptor.
        const SH = 0b11 << 7;
        const PA = ((1 << 40) - 1) << 12;
        /// Memory attributes for the returned output address. This field uses the same encoding as the Attr<n> fields in MAIR_EL1.
        ///
        /// The value returned in this field can be the resulting attribute that is actually implemented by the
        /// implementation, as determined by any permitted implementation choices and any applicable
        /// configuration bits, instead of the value that appears in the Translation table descriptor.
        const ATTR = ((1 << 8) - 1) << 56;
    }

    pub struct ParEl1Failure: u64 {
        /// Indicates whether the instruction performed a successful address translation. 1 for the failure case
        const FAULT = 0b1;
        /// Fault status code, as shown in the Data Abort exception ESR encoding.
        const FST = 0b111111 << 1;
    }
}

/// A descriptor in a non-last level translation table that may point to further translation tables or an output block translation
#[repr(C)]
#[derive(Copy, Clone)]
pub union TranslationDescriptor {
    pub table: TableDescriptor,
    pub leaf: LeafDescriptor,
}

/// A translation table.
///
/// Note that alignment varies based on the configured translation process, and so must be checked at runtime.
#[repr(C)]
pub struct TranslationTable(pub [TranslationDescriptor]);

/// A last level translation table
#[repr(C)]
pub struct LeafTable(pub [LeafDescriptor]);

impl TranslationTable {
    pub fn from_array<const N: usize>(arr: &[TranslationDescriptor; N]) -> &Self {
        unsafe { &*(&arr[..] as *const [_] as *const TranslationTable) }
    }
    pub fn from_array_mut<const N: usize>(arr: &mut [TranslationDescriptor; N]) -> &mut Self {
        unsafe { &mut *(&mut arr[..] as *mut [_] as *mut TranslationTable) }
    }
}

impl LeafTable {
    pub fn from_array<const N: usize>(arr: &[LeafDescriptor; N]) -> &Self {
        unsafe { &*(&arr[..] as *const [_] as *const LeafTable) }
    }
    pub fn from_array_mut<const N: usize>(arr: &mut [LeafDescriptor; N]) -> &mut Self {
        unsafe { &mut *(&mut arr[..] as *mut [_] as *mut LeafTable) }
    }
}

enum PageSize {
    Size4KiB,
    Size16KiB,
    Size64KiB,
}

impl TcrEl1 {
    const fn set_t0sz(self, t0sz: u8) -> Self {
        assert!(t0sz < (1 << 6), "field size mismatch");
        self.difference(Self::T0SZ)
            .union(Self::from_bits_retain(t0sz as u64))
    }

    const fn set_irgn0(self, irgn0: u8) -> Self {
        assert!(irgn0 < (1 << 2), "field size mismatch");
        self.difference(Self::IRGN0)
            .union(Self::from_bits_retain((irgn0 as u64) << 8))
    }

    const fn set_orgn0(self, orgn0: u8) -> Self {
        assert!(orgn0 < (1 << 2), "field size mismatch");
        self.difference(Self::ORGN0)
            .union(Self::from_bits_retain((orgn0 as u64) << 10))
    }

    const fn set_sh0(self, sh0: u8) -> Self {
        assert!(sh0 < (1 << 2), "field size mismatch");
        self.difference(Self::SH0)
            .union(Self::from_bits_retain((sh0 as u64) << 12))
    }

    const fn set_tg0(self, tg0: PageSize) -> Self {
        let tg0 = match tg0 {
            PageSize::Size4KiB => 0b00,
            PageSize::Size16KiB => 0b10,
            PageSize::Size64KiB => 0b01,
        };
        assert!(tg0 < (1 << 2), "field size mismatch");
        assert!(tg0 != 0b11, "reserved encoding");
        self.difference(Self::TG0)
            .union(Self::from_bits_retain((tg0 as u64) << 14))
    }

    const fn set_t1sz(self, t1sz: u8) -> Self {
        assert!(t1sz < (1 << 6));
        self.difference(Self::T1SZ)
            .union(Self::from_bits_retain((t1sz as u64) << 16))
    }

    const fn set_irgn1(self, irgn1: u8) -> Self {
        assert!(irgn1 < (1 << 2), "field size mismatch");
        self.difference(Self::IRGN1)
            .union(Self::from_bits_retain((irgn1 as u64) << 24))
    }

    const fn set_orgn1(self, orgn1: u8) -> Self {
        assert!(orgn1 < (1 << 2), "field size mismatch");
        self.difference(Self::ORGN1)
            .union(Self::from_bits_retain((orgn1 as u64) << 26))
    }

    const fn set_sh1(self, sh1: u8) -> Self {
        assert!(sh1 < (1 << 2), "field size mismatch");
        self.difference(Self::SH1)
            .union(Self::from_bits_retain((sh1 as u64) << 28))
    }

    const fn set_tg1(self, tg1: PageSize) -> Self {
        let tg1 = match tg1 {
            PageSize::Size4KiB => 0b10,
            PageSize::Size16KiB => 0b01,
            PageSize::Size64KiB => 0b11,
        };
        assert!(tg1 < (1 << 2), "field size mismatch");
        assert!(tg1 != 0b00, "reserved encoding");
        self.difference(Self::TG1)
            .union(Self::from_bits_retain((tg1 as u64) << 30))
    }

    const fn set_ips(self, ips: u8) -> Self {
        assert!(ips < (1 << 3), "field size mismatch");
        assert!(ips != 0b111, "reserved encoding");
        self.difference(Self::IPS)
            .union(Self::from_bits_retain((ips as u64) << 32))
    }

    pub const fn default() -> Self {
        Self::empty()
            .set_t0sz(39) // 25 bits of address translation
            .difference(Self::EPD0)
            .set_irgn0(0b01)
            .set_orgn0(0b01)
            .set_sh0(0b10)
            .set_tg0(PageSize::Size4KiB)
            .set_t1sz(39) // 25 bits of address translation
            .difference(Self::A1)
            .difference(Self::EPD1)
            .set_irgn1(0b01)
            .set_orgn1(0b01)
            .set_sh1(0b10)
            .set_tg1(PageSize::Size4KiB)
            .set_ips(0b101)
            .union(Self::AS)
            .difference(Self::TBI0)
            .difference(Self::TBI1)
    }
}

impl TableDescriptor {
    const fn set_pa(self, pa: usize) -> Self {
        assert!(pa < (1 << 52), "field size mismatch");
        assert!(pa % (1 << 12) == 0, "alignment mismatch");
        self.difference(Self::NEXT_ADDR)
            .union(Self::from_bits_retain(pa as u64))
    }

    pub const fn is_valid(self) -> bool {
        self.contains(Self::VALID)
    }

    pub const fn new(pa: usize) -> Self {
        Self::empty()
            .union(Self::VALID)
            .union(Self::IS_TABLE_DESCRIPTOR)
            .set_pa(pa)
    }
}

impl LeafDescriptor {
    const fn set_sh(self, sh: u8) -> Self {
        assert!(sh != 0b01, "reserved encoding");
        assert!(sh < (1 << 2), "field size mismatch");
        self.difference(Self::SHAREABILITY)
            .union(Self::from_bits_retain((sh as u64) << 8))
    }

    pub const fn set_global(self) -> Self {
        self.difference(Self::NOT_GLOBAL)
    }

    const fn set_pa(self, pa: usize) -> Self {
        assert!(pa < (1 << 52), "field size mismatch");
        assert!(pa % (1 << 12) == 0, "alignment mismatch");
        self.difference(Self::OA)
            .union(Self::from_bits_retain(pa as u64))
    }

    pub const fn clear_pxn(self) -> Self {
        self.difference(Self::PXN)
    }

    pub const fn is_valid(self) -> bool {
        self.contains(Self::VALID)
    }

    pub const fn set_mair(self, mair: u8) -> Self {
        assert!(mair < (1 << 3), "field size mismatch");

        self.difference(Self::ATTR_IDX)
            .union(Self::from_bits_retain((mair as u64) << 2))
    }

    pub const fn new(pa: usize) -> Self {
        Self::empty()
            .union(Self::VALID)
            .union(Self::IS_PAGE_DESCRIPTOR)
            .difference(Self::UNPRIVILEGED_ACCESS)
            .difference(Self::READ_ONLY)
            .set_sh(0b10) // outer shareable
            .union(Self::ACCESSED)
            .union(Self::NOT_GLOBAL)
            .set_pa(pa)
            .union(Self::PXN)
            .union(Self::UXN)
    }
}

impl ParEl1Success {
    pub const fn base_pa(self) -> u64 {
        self.intersection(Self::PA).bits()
    }
}

#[inline]
pub fn at_s1e1r(va: usize) -> Result<ParEl1Success, ParEl1Failure> {
    let par_el1: u64;
    // When an address translation instruction is executed, explicit synchronization is required to guarantee the result is visible to subsequent direct reads of PAR_EL1.
    unsafe {
        asm! {
            "at s1e1r, {x}",
            "isb",
            "mrs {x}, par_el1",
            x = inlateout(reg) va => par_el1,
            options(readonly, preserves_flags, nostack)
        }
    };
    if par_el1 & 1 == 0 {
        // No fault
        Ok(ParEl1Success::from_bits_retain(par_el1))
    } else {
        // Fault
        Err(ParEl1Failure::from_bits_retain(par_el1))
    }
}

#[inline]
pub fn at_s1e0r(va: usize) -> Result<ParEl1Success, ParEl1Failure> {
    let par_el1: u64;
    // When an address translation instruction is executed, explicit synchronization is required to guarantee the result is visible to subsequent direct reads of PAR_EL1.
    unsafe {
        asm! {
            "at s1e0r, {x}",
            "isb",
            "mrs {x}, par_el1",
            x = inlateout(reg) va => par_el1,
            options(readonly, preserves_flags, nostack)
        }
    };
    if par_el1 & 1 == 0 {
        // No fault
        Ok(ParEl1Success::from_bits_retain(par_el1))
    } else {
        // Fault
        Err(ParEl1Failure::from_bits_retain(par_el1))
    }
}
