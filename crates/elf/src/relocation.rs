// https://refspecs.linuxfoundation.org/elf/gabi4+/ch4.reloc.html

use core::{
    error,
    fmt::{Display, Formatter},
};

use super::{elf_header, section_header, symbol, types::*, Elf};

pub enum Relocation<'a> {
    Relocation {
        r_offset: u64,
        r_info: Info,
        r_info_value: u64,
        r_type: Type,
        elf: &'a Elf<'a>,
    },
    RelocationAddend {
        r_offset: u64,
        r_info: Info,
        r_info_value: u64,
        r_type: Type,
        r_addend: i64,
        elf: &'a Elf<'a>,
    },
}

impl<'a> Relocation<'a> {
    fn elf(&self) -> &'a Elf<'a> {
        match self {
            Self::Relocation { elf, .. } => elf,
            Self::RelocationAddend { elf, .. } => elf,
        }
    }

    pub fn r_offset(&self) -> u64 {
        match self {
            Self::Relocation { r_offset, .. } => *r_offset,
            Self::RelocationAddend { r_offset, .. } => *r_offset,
        }
    }

    pub fn r_info(&self) -> Info {
        match self {
            Self::Relocation { r_info, .. } => *r_info,
            Self::RelocationAddend { r_info, .. } => *r_info,
        }
    }

    pub fn r_info_value(&self) -> u64 {
        match self {
            Self::Relocation { r_info_value, .. } => *r_info_value,
            Self::RelocationAddend { r_info_value, .. } => *r_info_value,
        }
    }

    pub fn r_type(&self) -> Type {
        match self {
            Self::Relocation { r_type, .. } => *r_type,
            Self::RelocationAddend { r_type, .. } => *r_type,
        }
    }

    fn r_sym_value(&self) -> u32 {
        self.r_info().r_sym()
    }

    pub fn r_sym(
        &self,
        symbol_table: &section_header::SectionHeader,
    ) -> Result<symbol::Symbol, symbol::SymbolError> {
        let symbol_table_start = symbol_table.sh_offset as usize;
        let symbol_table_entry_size = symbol_table.sh_entsize as usize;
        let symbol_start =
            symbol_table_entry_size * self.r_sym_value() as usize + symbol_table_start;
        symbol::Symbol::new(self.elf(), symbol_start)
    }

    pub fn r_addend(&self) -> i64 {
        match self {
            Self::Relocation { .. } => 0,
            Self::RelocationAddend { r_addend, .. } => *r_addend,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct Elf32Rel {
    r_offset: Elf32Addr,
    r_info: Elf32Word,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct Elf32Rela {
    r_offset: Elf32Addr,
    r_info: Elf32Word,
    r_addend: Elf32Sword,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct Elf64Rel {
    r_offset: Elf64Addr,
    r_info: Elf64Xword,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct Elf64Rela {
    r_offset: Elf64Addr,
    r_info: Elf64Xword,
    r_addend: Elf64Sxword,
}

unsafe impl bytemuck::Zeroable for Elf32Rel {}
unsafe impl bytemuck::AnyBitPattern for Elf32Rel {}

unsafe impl bytemuck::Zeroable for Elf32Rela {}
unsafe impl bytemuck::AnyBitPattern for Elf32Rela {}

unsafe impl bytemuck::Zeroable for Elf64Rel {}
unsafe impl bytemuck::AnyBitPattern for Elf64Rel {}

unsafe impl bytemuck::Zeroable for Elf64Rela {}
unsafe impl bytemuck::AnyBitPattern for Elf64Rela {}

#[derive(Debug, Clone, Copy)]
pub enum Info {
    Elf32RelocationInfo(u32),
    Elf64RelocationInfo(u64),
}

impl Info {
    pub fn r_sym(&self) -> u32 {
        match self {
            Self::Elf32RelocationInfo(i) => i >> 8,
            Self::Elf64RelocationInfo(i) => (i >> 32) as u32,
        }
    }
    pub fn r_type(&self) -> u32 {
        match self {
            Self::Elf32RelocationInfo(i) => i & 0xff,
            Self::Elf64RelocationInfo(i) => (i & 0xffff_ffff) as u32,
        }
    }

    pub fn elf32_r_info(symbol_index: u32, relocation_type: u8) -> Self {
        Self::Elf32RelocationInfo((symbol_index << 8) + relocation_type as u32)
    }

    pub fn elf64_r_info(symbol_index: u32, relocation_type: u32) -> Self {
        Self::Elf64RelocationInfo(((symbol_index as Elf64Xword) << 32) + relocation_type as u64)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AArch64Type {
    None,
    P32Abs32,
    P32Copy,
    P32GlobDat,
    P32JumpSlot,
    P32Relative,
    P32TlsDtpMod,
    P32TlsDtpRel,
    P32TlsTprel,
    P32TlsDesc,
    P32Irelative,
    Abs64,
    Abs32,
    Abs16,
    Prel64,
    Prel32,
    Prel16,
    MovwUabsG0,
    MovwUabsG0Nc,
    MovwUabsG1,
    MovwUabsG1Nc,
    MovwUabsG2,
    MovwUabsG2Nc,
    MovwUabsG3,
    MovwSabsG0,
    MovwSabsG1,
    MovwSabsG2,
    LdPrelLo19,
    AdrPrelLo21,
    AdrPrelPgHi21,
    AdrPrelPgHi21Nc,
    AddAbsLo12Nc,
    Ldst8AbsLo12Nc,
    Tstbr14,
    Conbr19,
    Jump26,
    Call26,
    Ldst16AbsLo12Nc,
    Ldst32AbsLo12Nc,
    Ldst64AbsLo12Nc,
    MovwPrelG0,
    MovwPrelG0Nc,
    MovwPrelG1,
    MovwPrelG1Nc,
    MovwPrelG2,
    MovwPrelG2Nc,
    MovwPrelG3,
    LdSt128AbsLo12Nc,
    MovwGotoffG0,
    MovwGotoffG0Nc,
    MovwGotoffG1,
    MovwGotoffG1Nc,
    MovwGotoffG2,
    MovwGotoffG2Nc,
    MovwGotoffG3,
    Gotrel64,
    Gotrel32,
    GotLdPrelLo19,
    Ld64GotoffLo15,
    AdrGotPage,
    Ld64GotLo12Nc,
    Ld64GotPageLo15,
    TlsgdAdrPrel21,
    TlsgdAdrPage21,
    TlsgdAddLo12Nc,
    TlsgdMovwG1,
    TlsgdMovwG0Nc,
    TsldAdrPrel21,
    TsldAdrPage21,
    TsldAddLo12Nc,
    TsldMovwG1,
    TsldMovwG0Nc,
    TsldLdPrel19,
    TsldMovwDtprelG2,
    TsldMovwDtprelG1,
    TsldMovwDtprelG1Nc,
    TsldMovwDtprelG0,
    TsldMovwDtprelG0Nc,
    TsldAddDtprelHi12,
    TsldAddDtprelLo12,
    TsldAddDtprelLo12Nc,
    TsldLdSt8DtprelLo12,
    TsldLdSt8DtprelLo12Nc,
    TsldLdSt16DtprelLo12,
    TsldLdSt16DtprelLo12Nc,
    TsldLdSt32DtprelLo12,
    TsldLdSt32DtprelLo12Nc,
    TsldLdSt64DtprelLo12,
    TsldLdSt64DtprelLo12Nc,
    TlsieMovwGottprelG1,
    TlsieMovwGottprelG0Nc,
    TlsieAdrGottprelPage21,
    TlsieLd64GottprelLo12Nc,
    TlsieLdGottprelPrel19,
    TlsleMovwTprelG2,
    TlsleMovwTprelG1,
    TlsleMovwTprelG1Nc,
    TlsleMovwTprelG0,
    TlsleMovwTprelG0Nc,
    TlsleAddTprelHi12,
    TlsleAddTprelLo12,
    TlsleAddTprelLo12Nc,
    TlsleLdSt8TprelLo12,
    TlsleLdSt8TprelLo12Nc,
    TlsleLdSt16TprelLo12,
    TlsleLdSt16TprelLo12Nc,
    TlsleLdSt32TprelLo12,
    TlsleLdSt32TprelLo12Nc,
    TlsleLdSt64TprelLo12,
    TlsleLdSt64TprelLo12Nc,
    TlsdescLdPreLo19,
    TlsdescAdrPrel21,
    TlsdescAdrPage21,
    TlsdescLd64Lo12,
    TlsdescAddLo12,
    TlsdescOffG1,
    TlsdescOffG0Nc,
    TlsdescLdr,
    TlsdescAdd,
    TlsdescCall,
    TlsleLdst128TprelLo12,
    TlsleLdst128TprelLo12Nc,
    TlsldLdst128DtprelLo12,
    TlsldLdst128DtprelLo12Nc,
    Copy,
    GlobDat,
    JumpSlot,
    Relative,
    TlsDtpmod,
    TlsDtprel,
    TlsTprel,
    Tlsdesc,
    Irelative,
}

impl TryFrom<Info> for AArch64Type {
    type Error = RelocationError;
    fn try_from(r_info: Info) -> Result<Self, Self::Error> {
        match r_info.r_type() {
            0 => Ok(Self::None),
            1 => Ok(Self::P32Abs32),
            180 => Ok(Self::P32Copy),
            181 => Ok(Self::P32GlobDat),
            182 => Ok(Self::P32JumpSlot),
            183 => Ok(Self::P32Relative),
            184 => Ok(Self::P32TlsDtpMod),
            185 => Ok(Self::P32TlsDtpRel),
            186 => Ok(Self::P32TlsTprel),
            187 => Ok(Self::P32TlsDesc),
            188 => Ok(Self::P32Irelative),
            257 => Ok(Self::Abs64),
            258 => Ok(Self::Abs32),
            259 => Ok(Self::Abs16),
            260 => Ok(Self::Prel64),
            261 => Ok(Self::Prel32),
            262 => Ok(Self::Prel16),
            263 => Ok(Self::MovwUabsG0),
            264 => Ok(Self::MovwUabsG0Nc),
            265 => Ok(Self::MovwUabsG1),
            266 => Ok(Self::MovwUabsG1Nc),
            267 => Ok(Self::MovwUabsG2),
            268 => Ok(Self::MovwUabsG2Nc),
            269 => Ok(Self::MovwUabsG3),
            270 => Ok(Self::MovwSabsG0),
            271 => Ok(Self::MovwSabsG1),
            272 => Ok(Self::MovwSabsG2),
            273 => Ok(Self::LdPrelLo19),
            274 => Ok(Self::AdrPrelLo21),
            275 => Ok(Self::AdrPrelPgHi21),
            276 => Ok(Self::AdrPrelPgHi21Nc),
            277 => Ok(Self::AddAbsLo12Nc),
            278 => Ok(Self::Ldst8AbsLo12Nc),
            279 => Ok(Self::Tstbr14),
            280 => Ok(Self::Conbr19),
            282 => Ok(Self::Jump26),
            283 => Ok(Self::Call26),
            284 => Ok(Self::Ldst16AbsLo12Nc),
            285 => Ok(Self::Ldst32AbsLo12Nc),
            286 => Ok(Self::Ldst64AbsLo12Nc),
            287 => Ok(Self::MovwPrelG0),
            288 => Ok(Self::MovwPrelG0Nc),
            289 => Ok(Self::MovwPrelG1),
            290 => Ok(Self::MovwPrelG1Nc),
            291 => Ok(Self::MovwPrelG2),
            292 => Ok(Self::MovwPrelG2Nc),
            293 => Ok(Self::MovwPrelG3),
            299 => Ok(Self::LdSt128AbsLo12Nc),
            300 => Ok(Self::MovwGotoffG0),
            301 => Ok(Self::MovwGotoffG0Nc),
            302 => Ok(Self::MovwGotoffG1),
            303 => Ok(Self::MovwGotoffG1Nc),
            304 => Ok(Self::MovwGotoffG2),
            305 => Ok(Self::MovwGotoffG2Nc),
            306 => Ok(Self::MovwGotoffG3),
            307 => Ok(Self::Gotrel64),
            308 => Ok(Self::Gotrel32),
            309 => Ok(Self::GotLdPrelLo19),
            310 => Ok(Self::Ld64GotoffLo15),
            311 => Ok(Self::AdrGotPage),
            312 => Ok(Self::Ld64GotLo12Nc),
            313 => Ok(Self::Ld64GotPageLo15),
            512 => Ok(Self::TlsgdAdrPrel21),
            513 => Ok(Self::TlsgdAdrPage21),
            514 => Ok(Self::TlsgdAddLo12Nc),
            515 => Ok(Self::TlsgdMovwG1),
            516 => Ok(Self::TlsgdMovwG0Nc),
            517 => Ok(Self::TsldAdrPrel21),
            518 => Ok(Self::TsldAdrPage21),
            519 => Ok(Self::TsldAddLo12Nc),
            520 => Ok(Self::TsldMovwG1),
            521 => Ok(Self::TsldMovwG0Nc),
            522 => Ok(Self::TsldLdPrel19),
            523 => Ok(Self::TsldMovwDtprelG2),
            524 => Ok(Self::TsldMovwDtprelG1),
            525 => Ok(Self::TsldMovwDtprelG1Nc),
            526 => Ok(Self::TsldMovwDtprelG0),
            527 => Ok(Self::TsldMovwDtprelG0Nc),
            528 => Ok(Self::TsldAddDtprelHi12),
            529 => Ok(Self::TsldAddDtprelLo12),
            530 => Ok(Self::TsldAddDtprelLo12Nc),
            531 => Ok(Self::TsldLdSt8DtprelLo12),
            532 => Ok(Self::TsldLdSt8DtprelLo12Nc),
            533 => Ok(Self::TsldLdSt16DtprelLo12),
            534 => Ok(Self::TsldLdSt16DtprelLo12Nc),
            535 => Ok(Self::TsldLdSt32DtprelLo12),
            536 => Ok(Self::TsldLdSt32DtprelLo12Nc),
            537 => Ok(Self::TsldLdSt64DtprelLo12),
            538 => Ok(Self::TsldLdSt64DtprelLo12Nc),
            539 => Ok(Self::TlsieMovwGottprelG1),
            540 => Ok(Self::TlsieMovwGottprelG0Nc),
            541 => Ok(Self::TlsieAdrGottprelPage21),
            542 => Ok(Self::TlsieLd64GottprelLo12Nc),
            543 => Ok(Self::TlsieLdGottprelPrel19),
            544 => Ok(Self::TlsleMovwTprelG2),
            545 => Ok(Self::TlsleMovwTprelG1),
            546 => Ok(Self::TlsleMovwTprelG1Nc),
            547 => Ok(Self::TlsleMovwTprelG0),
            548 => Ok(Self::TlsleMovwTprelG0Nc),
            549 => Ok(Self::TlsleAddTprelHi12),
            550 => Ok(Self::TlsleAddTprelLo12),
            551 => Ok(Self::TlsleAddTprelLo12Nc),
            552 => Ok(Self::TlsleLdSt8TprelLo12),
            553 => Ok(Self::TlsleLdSt8TprelLo12Nc),
            554 => Ok(Self::TlsleLdSt16TprelLo12),
            555 => Ok(Self::TlsleLdSt16TprelLo12Nc),
            556 => Ok(Self::TlsleLdSt32TprelLo12),
            557 => Ok(Self::TlsleLdSt32TprelLo12Nc),
            558 => Ok(Self::TlsleLdSt64TprelLo12),
            559 => Ok(Self::TlsleLdSt64TprelLo12Nc),
            560 => Ok(Self::TlsdescLdPreLo19),
            561 => Ok(Self::TlsdescAdrPrel21),
            562 => Ok(Self::TlsdescAdrPage21),
            563 => Ok(Self::TlsdescLd64Lo12),
            564 => Ok(Self::TlsdescAddLo12),
            565 => Ok(Self::TlsdescOffG1),
            566 => Ok(Self::TlsdescOffG0Nc),
            567 => Ok(Self::TlsdescLdr),
            568 => Ok(Self::TlsdescAdd),
            569 => Ok(Self::TlsdescCall),
            570 => Ok(Self::TlsleLdst128TprelLo12),
            571 => Ok(Self::TlsleLdst128TprelLo12Nc),
            572 => Ok(Self::TlsldLdst128DtprelLo12),
            573 => Ok(Self::TlsldLdst128DtprelLo12Nc),
            1024 => Ok(Self::Copy),
            1025 => Ok(Self::GlobDat),
            1026 => Ok(Self::JumpSlot),
            1027 => Ok(Self::Relative),
            1028 => Ok(Self::TlsDtpmod),
            1029 => Ok(Self::TlsDtprel),
            1030 => Ok(Self::TlsTprel),
            1031 => Ok(Self::Tlsdesc),
            1032 => Ok(Self::Irelative),
            _ => Err(Self::Error::UnknownType),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ARMType {
    None,
    Pc24,
    Abs32,
    Rel32,
    Pc13,
    Abs16,
    Abs12,
    ThmAbs5,
    Abs8,
    Sbrel32,
    ThmPc22,
    ThmPc8,
    AmpVcall9,
    // Swi24, // OBSOLETE, value reassigned to TLS_DESC
    TlsDesc,
    ThmSwi8,
    Xpc25,
    ThmXpc22,
    TlsDtpmod32,
    TlsDtpoff32,
    TlsTpoff32,
    Copy,
    GlobDat,
    JumpSlot,
    Relative,
    Gotoff,
    Gotpc,
    Got32,
    Plt32,
    Call,
    Jump24,
    ThmJump24,
    BaseAbs,
    AluPcrel7_0,
    AluPcrel15_8,
    AluPcrel23_15,
    LdrSbrel11_0,
    AluSbrel19_12,
    AluSbrel27_20,
    Target1,
    Sbrel31,
    V4bx,
    Target2,
    Prel31,
    MovwAbsNc,
    MovtAbs,
    MovwPrelNc,
    MovtPrel,
    ThmMovwAbsNc,
    ThmMovtAbs,
    ThmMovwPrelNc,
    ThmMovtPrel,
    ThmJump19,
    ThmJump6,
    ThmAluPrel11_0,
    ThmPc12,
    Abs32Noi,
    Rel32Noi,
    AluPcG0Nc,
    AluPcG0,
    AluPcG1Nc,
    AluPcG1,
    AluPcG2,
    LdrPcG1,
    LdrPcG2,
    LdrsPcG0,
    LdrsPcG1,
    LdrsPcG2,
    LdcPcG0,
    LdcPcG1,
    LdcPcG2,
    AluSbG0Nc,
    AluSbG0,
    AluSbG1Nc,
    AluSbG1,
    AluSbG2,
    LdrSbG0,
    LdrSbG1,
    LdrSbG2,
    LdrsSbG0,
    LdrsSbG1,
    LdrsSbG2,
    LdcSbG0,
    LdcSbG1,
    LdcSbG2,
    MovwBrelNc,
    MovtBrel,
    MovwBrel,
    ThmMowBrelNc,
    ThmMovtBrel,
    ThmMovwBrel,
    TlsGotdesc,
    TlsCall,
    TlsDescseq,
    ThmTlsCall,
    Plt32Abs,
    GotAbs,
    GotPrel,
    GotBrel12,
    Gotoff12,
    Gotrelax,
    GnuVtentry,
    GnuVtinherit,
    ThmPc11,
    ThmPc9,
    TlsGd32,
    TlsLdm32,
    TlsLdo32,
    TlsIe32,
    TlsLe32,
    TlsLdo12,
    TlsLe12,
    TlsIe12Gp,
    MeToo,
    // ThmTlsDescseq, // same as TlsDescseq16
    ThmTlsDescseq16,
    ThmTlsDescseq32,
    ThmGotBrel12,
    Irelative,
    Rxpc25,
    RsBrel32,
    ThmRpc22,
    Rrel32,
    Rabs22,
    Rpc24,
    Rbase,
}

impl TryFrom<Info> for ARMType {
    type Error = RelocationError;
    fn try_from(r_info: Info) -> Result<Self, Self::Error> {
        match r_info.r_type() {
            0 => Ok(Self::None),
            1 => Ok(Self::Pc24),
            2 => Ok(Self::Abs32),
            3 => Ok(Self::Rel32),
            4 => Ok(Self::Pc13),
            5 => Ok(Self::Abs16),
            6 => Ok(Self::Abs12),
            7 => Ok(Self::ThmAbs5),
            8 => Ok(Self::Abs8),
            9 => Ok(Self::Sbrel32),
            10 => Ok(Self::ThmPc22),
            11 => Ok(Self::ThmPc8),
            12 => Ok(Self::AmpVcall9),
            13 => Ok(Self::TlsDesc),
            14 => Ok(Self::ThmSwi8),
            15 => Ok(Self::Xpc25),
            16 => Ok(Self::ThmXpc22),
            17 => Ok(Self::TlsDtpmod32),
            18 => Ok(Self::TlsDtpoff32),
            19 => Ok(Self::TlsTpoff32),
            20 => Ok(Self::Copy),
            21 => Ok(Self::GlobDat),
            22 => Ok(Self::JumpSlot),
            23 => Ok(Self::Relative),
            24 => Ok(Self::Gotoff),
            25 => Ok(Self::Gotpc),
            26 => Ok(Self::Got32),
            27 => Ok(Self::Plt32),
            28 => Ok(Self::Call),
            29 => Ok(Self::Jump24),
            30 => Ok(Self::ThmJump24),
            31 => Ok(Self::BaseAbs),
            32 => Ok(Self::AluPcrel7_0),
            33 => Ok(Self::AluPcrel15_8),
            34 => Ok(Self::AluPcrel23_15),
            35 => Ok(Self::LdrSbrel11_0),
            36 => Ok(Self::AluSbrel19_12),
            37 => Ok(Self::AluSbrel27_20),
            38 => Ok(Self::Target1),
            39 => Ok(Self::Sbrel31),
            40 => Ok(Self::V4bx),
            41 => Ok(Self::Target2),
            42 => Ok(Self::Prel31),
            43 => Ok(Self::MovwAbsNc),
            44 => Ok(Self::MovtAbs),
            45 => Ok(Self::MovwPrelNc),
            46 => Ok(Self::MovtPrel),
            47 => Ok(Self::ThmMovwAbsNc),
            48 => Ok(Self::ThmMovtAbs),
            49 => Ok(Self::ThmMovwPrelNc),
            50 => Ok(Self::ThmMovtPrel),
            51 => Ok(Self::ThmJump19),
            52 => Ok(Self::ThmJump6),
            53 => Ok(Self::ThmAluPrel11_0),
            54 => Ok(Self::ThmPc12),
            55 => Ok(Self::Abs32Noi),
            56 => Ok(Self::Rel32Noi),
            57 => Ok(Self::AluPcG0Nc),
            58 => Ok(Self::AluPcG0),
            59 => Ok(Self::AluPcG1Nc),
            60 => Ok(Self::AluPcG1),
            61 => Ok(Self::AluPcG2),
            62 => Ok(Self::LdrPcG1),
            63 => Ok(Self::LdrPcG2),
            64 => Ok(Self::LdrsPcG0),
            65 => Ok(Self::LdrsPcG1),
            66 => Ok(Self::LdrsPcG2),
            67 => Ok(Self::LdcPcG0),
            68 => Ok(Self::LdcPcG1),
            69 => Ok(Self::LdcPcG2),
            70 => Ok(Self::AluSbG0Nc),
            71 => Ok(Self::AluSbG0),
            72 => Ok(Self::AluSbG1Nc),
            73 => Ok(Self::AluSbG1),
            74 => Ok(Self::AluSbG2),
            75 => Ok(Self::LdrSbG0),
            76 => Ok(Self::LdrSbG1),
            77 => Ok(Self::LdrSbG2),
            78 => Ok(Self::LdrsSbG0),
            79 => Ok(Self::LdrsSbG1),
            80 => Ok(Self::LdrsSbG2),
            81 => Ok(Self::LdcSbG0),
            82 => Ok(Self::LdcSbG1),
            83 => Ok(Self::LdcSbG2),
            84 => Ok(Self::MovwBrelNc),
            85 => Ok(Self::MovtBrel),
            86 => Ok(Self::MovwBrel),
            87 => Ok(Self::ThmMowBrelNc),
            88 => Ok(Self::ThmMovtBrel),
            89 => Ok(Self::ThmMovwBrel),
            90 => Ok(Self::TlsGotdesc),
            91 => Ok(Self::TlsCall),
            92 => Ok(Self::TlsDescseq),
            93 => Ok(Self::ThmTlsCall),
            94 => Ok(Self::Plt32Abs),
            95 => Ok(Self::GotAbs),
            96 => Ok(Self::GotPrel),
            97 => Ok(Self::GotBrel12),
            98 => Ok(Self::Gotoff12),
            99 => Ok(Self::Gotrelax),
            100 => Ok(Self::GnuVtentry),
            101 => Ok(Self::GnuVtinherit),
            102 => Ok(Self::ThmPc11),
            103 => Ok(Self::ThmPc9),
            104 => Ok(Self::TlsGd32),
            105 => Ok(Self::TlsLdm32),
            106 => Ok(Self::TlsLdo32),
            107 => Ok(Self::TlsIe32),
            108 => Ok(Self::TlsLe32),
            109 => Ok(Self::TlsLdo12),
            110 => Ok(Self::TlsLe12),
            111 => Ok(Self::TlsIe12Gp),
            128 => Ok(Self::MeToo),
            129 => Ok(Self::ThmTlsDescseq16),
            130 => Ok(Self::ThmTlsDescseq32),
            131 => Ok(Self::ThmGotBrel12),
            160 => Ok(Self::Irelative),
            249 => Ok(Self::Rxpc25),
            250 => Ok(Self::RsBrel32),
            251 => Ok(Self::ThmRpc22),
            252 => Ok(Self::Rrel32),
            253 => Ok(Self::Rabs22),
            254 => Ok(Self::Rpc24),
            255 => Ok(Self::Rbase),
            _ => Err(Self::Error::UnknownType),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum X86Type {
    None,
    _32,
    Pc32,
    Got32,
    Plt32,
    Copy,
    GlobDat,
    JmpSlot,
    Relative,
    Gotoff,
    Gotpc,
    _32plt,
    TlsTpoff,
    TlsIe,
    TlsGotie,
    TlsLe,
    TlsGd,
    TlsLdm,
    _16,
    Pc16,
    _8,
    Pc8,
    TlsGd32,
    TlsGdPush,
    TlsGdCall,
    TlsGdPop,
    TlsLdm32,
    TlsLdmPush,
    TlsLdmCall,
    TlsLdmPop,
    TldLdo32,
    TlsIe32,
    TlsLe32,
    TlsDtpmod32,
    TlsDtpoff32,
    TlsTpoff32,
    Size32,
    TlsGotdesc,
    TlsDescCall,
    TlsDesc,
    Irelative,
    Got32x,
}

impl TryFrom<Info> for X86Type {
    type Error = RelocationError;
    fn try_from(r_info: Info) -> Result<Self, Self::Error> {
        match r_info.r_type() {
            0 => Ok(Self::None),
            1 => Ok(Self::_32),
            2 => Ok(Self::Pc32),
            3 => Ok(Self::Got32),
            4 => Ok(Self::Plt32),
            5 => Ok(Self::Copy),
            6 => Ok(Self::GlobDat),
            7 => Ok(Self::JmpSlot),
            8 => Ok(Self::Relative),
            9 => Ok(Self::Gotoff),
            10 => Ok(Self::Gotpc),
            11 => Ok(Self::_32plt),
            14 => Ok(Self::TlsTpoff),
            15 => Ok(Self::TlsIe),
            16 => Ok(Self::TlsGotie),
            17 => Ok(Self::TlsLe),
            18 => Ok(Self::TlsGd),
            19 => Ok(Self::TlsLdm),
            20 => Ok(Self::_16),
            21 => Ok(Self::Pc16),
            22 => Ok(Self::_8),
            23 => Ok(Self::Pc8),
            24 => Ok(Self::TlsGd32),
            25 => Ok(Self::TlsGdPush),
            26 => Ok(Self::TlsGdCall),
            27 => Ok(Self::TlsGdPop),
            28 => Ok(Self::TlsLdm32),
            29 => Ok(Self::TlsLdmPush),
            30 => Ok(Self::TlsLdmCall),
            31 => Ok(Self::TlsLdmPop),
            32 => Ok(Self::TldLdo32),
            33 => Ok(Self::TlsIe32),
            34 => Ok(Self::TlsLe32),
            35 => Ok(Self::TlsDtpmod32),
            36 => Ok(Self::TlsDtpoff32),
            37 => Ok(Self::TlsTpoff32),
            38 => Ok(Self::Size32),
            39 => Ok(Self::TlsGotdesc),
            40 => Ok(Self::TlsDescCall),
            41 => Ok(Self::TlsDesc),
            42 => Ok(Self::Irelative),
            43 => Ok(Self::Got32x),
            _ => Err(Self::Error::UnknownType),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum X86_64Type {
    None,
    _64,
    Pc32,
    Got32,
    Plt32,
    Copy,
    GlobDat,
    JmpSlot,
    Relative,
    Gotpcrel,
    _32,
    _32s,
    _16,
    Pc16,
    _8,
    Pc8,
    Dtpmod64,
    Dtpoff64,
    Tpoff64,
    Tlsgd,
    Tlsld,
    Dtpoff32,
    Gottpoff,
    Tpoff32,
    Pc64,
    Gotoff64,
    Gotpc32,
    Got64,
    Gotpcrel64,
    Gotpc64,
    Gotplt64,
    Pltoff64,
    Size32,
    Size64,
    Gotpc32Tlsdesc,
    TlsdescCall,
    Tlsdesc,
    Irelative,
    Relative64,
    Gotpcrelx,
    RexGotpcrelx,
}

impl TryFrom<Info> for X86_64Type {
    type Error = RelocationError;
    fn try_from(r_info: Info) -> Result<Self, Self::Error> {
        match r_info.r_type() {
            0 => Ok(Self::None),
            1 => Ok(Self::_64),
            2 => Ok(Self::Pc32),
            3 => Ok(Self::Got32),
            4 => Ok(Self::Plt32),
            5 => Ok(Self::Copy),
            6 => Ok(Self::GlobDat),
            7 => Ok(Self::JmpSlot),
            8 => Ok(Self::Relative),
            9 => Ok(Self::Gotpcrel),
            10 => Ok(Self::_32),
            11 => Ok(Self::_32s),
            12 => Ok(Self::_16),
            13 => Ok(Self::Pc16),
            14 => Ok(Self::_8),
            15 => Ok(Self::Pc8),
            16 => Ok(Self::Dtpmod64),
            17 => Ok(Self::Dtpoff64),
            18 => Ok(Self::Tpoff64),
            19 => Ok(Self::Tlsgd),
            20 => Ok(Self::Tlsld),
            21 => Ok(Self::Dtpoff32),
            22 => Ok(Self::Gottpoff),
            23 => Ok(Self::Tpoff32),
            24 => Ok(Self::Pc64),
            25 => Ok(Self::Gotoff64),
            26 => Ok(Self::Gotpc32),
            27 => Ok(Self::Got64),
            28 => Ok(Self::Gotpcrel64),
            29 => Ok(Self::Gotpc64),
            30 => Ok(Self::Gotplt64),
            31 => Ok(Self::Pltoff64),
            32 => Ok(Self::Size32),
            33 => Ok(Self::Size64),
            34 => Ok(Self::Gotpc32Tlsdesc),
            35 => Ok(Self::TlsdescCall),
            36 => Ok(Self::Tlsdesc),
            37 => Ok(Self::Irelative),
            38 => Ok(Self::Relative64),
            41 => Ok(Self::Gotpcrelx),
            42 => Ok(Self::RexGotpcrelx),
            _ => Err(Self::Error::UnknownType),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Type {
    AArch64(AArch64Type),
    ARM(ARMType),
    X86(X86Type),
    X86_64(X86_64Type),
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AArch64(t) => write!(f, "{:?}", t),
            Self::ARM(t) => write!(f, "{:?}", t),
            Self::X86(t) => write!(f, "{:?}", t),
            Self::X86_64(t) => write!(f, "{:?}", t),
        }
    }
}

#[derive(Debug)]
pub enum RelocationError {
    InvalidLength,
    IncorrectSectionType,
    UnimplementedArchitecture,
    UnknownType,
}

impl Display for RelocationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidLength => write!(f, "Invalid length"),
            Self::IncorrectSectionType => write!(f, "Incorrect section type"),
            Self::UnimplementedArchitecture => write!(f, "Unimplemented architecture"),
            Self::UnknownType => write!(f, "Unknown type"),
        }
    }
}

impl error::Error for RelocationError {}

impl<'a> Relocation<'a> {
    pub(crate) fn new_rel32(elf: &'a Elf<'a>, offset: usize) -> Result<Self, RelocationError> {
        let data = elf
            .file_data
            .get(offset..offset + size_of::<Elf32Rel>())
            .ok_or(RelocationError::InvalidLength)?;
        let relocation: Elf32Rel = bytemuck::pod_read_unaligned(data);

        let r_offset = relocation.r_offset as u64;
        let r_info = Info::Elf32RelocationInfo(relocation.r_info);
        let r_type = match elf.elf_header().e_machine() {
            elf_header::Machine::AArch64 => Type::AArch64(AArch64Type::try_from(r_info)?),
            elf_header::Machine::ARM => Type::ARM(ARMType::try_from(r_info)?),
            elf_header::Machine::I386 => Type::X86(X86Type::try_from(r_info)?),
            elf_header::Machine::X86_64 => Type::X86_64(X86_64Type::try_from(r_info)?),
            _ => return Err(RelocationError::UnimplementedArchitecture),
        };

        Ok(Self::Relocation {
            r_offset,
            r_info,
            r_info_value: relocation.r_info as u64,
            r_type,
            elf,
        })
    }

    pub(crate) fn new_rel64(elf: &'a Elf<'a>, offset: usize) -> Result<Self, RelocationError> {
        let data = elf
            .file_data
            .get(offset..offset + size_of::<Elf64Rel>())
            .ok_or(RelocationError::InvalidLength)?;
        let relocation: Elf64Rel = bytemuck::pod_read_unaligned(data);

        let r_offset = relocation.r_offset;
        let r_info = Info::Elf64RelocationInfo(relocation.r_info);
        let r_type = match elf.elf_header().e_machine() {
            elf_header::Machine::AArch64 => Type::AArch64(AArch64Type::try_from(r_info)?),
            elf_header::Machine::ARM => Type::ARM(ARMType::try_from(r_info)?),
            elf_header::Machine::I386 => Type::X86(X86Type::try_from(r_info)?),
            elf_header::Machine::X86_64 => Type::X86_64(X86_64Type::try_from(r_info)?),
            _ => return Err(RelocationError::UnimplementedArchitecture),
        };

        Ok(Self::Relocation {
            r_offset,
            r_info,
            r_info_value: relocation.r_info,
            r_type,
            elf,
        })
    }

    pub(crate) fn new_rela32(elf: &'a Elf<'a>, offset: usize) -> Result<Self, RelocationError> {
        let data = elf
            .file_data
            .get(offset..offset + size_of::<Elf32Rela>())
            .ok_or(RelocationError::InvalidLength)?;
        let relocation: Elf32Rela = bytemuck::pod_read_unaligned(data);

        let r_offset = relocation.r_offset as u64;
        let r_info = Info::Elf32RelocationInfo(relocation.r_info);
        let r_type = match elf.elf_header().e_machine() {
            elf_header::Machine::AArch64 => Type::AArch64(AArch64Type::try_from(r_info)?),
            elf_header::Machine::ARM => Type::ARM(ARMType::try_from(r_info)?),
            elf_header::Machine::I386 => Type::X86(X86Type::try_from(r_info)?),
            elf_header::Machine::X86_64 => Type::X86_64(X86_64Type::try_from(r_info)?),
            _ => return Err(RelocationError::UnimplementedArchitecture),
        };
        let r_addend = relocation.r_addend as i64;

        Ok(Self::RelocationAddend {
            r_offset,
            r_info,
            r_info_value: relocation.r_info as u64,
            r_type,
            r_addend,
            elf,
        })
    }

    pub(crate) fn new_rela64(elf: &'a Elf<'a>, offset: usize) -> Result<Self, RelocationError> {
        let data = elf
            .file_data
            .get(offset..offset + size_of::<Elf64Rela>())
            .ok_or(RelocationError::InvalidLength)?;
        let relocation: Elf64Rela = bytemuck::pod_read_unaligned(data);

        let r_offset = relocation.r_offset;
        let r_info = Info::Elf64RelocationInfo(relocation.r_info);
        let r_type = match elf.elf_header().e_machine() {
            elf_header::Machine::AArch64 => Type::AArch64(AArch64Type::try_from(r_info)?),
            elf_header::Machine::ARM => Type::ARM(ARMType::try_from(r_info)?),
            elf_header::Machine::I386 => Type::X86(X86Type::try_from(r_info)?),
            elf_header::Machine::X86_64 => Type::X86_64(X86_64Type::try_from(r_info)?),
            _ => return Err(RelocationError::UnimplementedArchitecture),
        };
        let r_addend = relocation.r_addend;

        Ok(Self::RelocationAddend {
            r_offset,
            r_info,
            r_info_value: relocation.r_info,
            r_type,
            r_addend,
            elf,
        })
    }
}
