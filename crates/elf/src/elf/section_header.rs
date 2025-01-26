// https://refspecs.linuxfoundation.org/elf/gabi4+/ch4.sheader.html

use core::{error, fmt::Display, str::Utf8Error};

use super::{elf_header, identity, types::*, Elf};

pub const SHN_UNDEF: u16 = 0;
pub const SHN_LORESERVE: u16 = 0xff00;
pub const SHN_LOPROC: u16 = 0xff00;
pub const SHN_HIPROC: u16 = 0xff1f;
pub const SHN_LOOS: u16 = 0xff20;
pub const SHN_HIOS: u16 = 0xff3f;
pub const SHN_ABS: u16 = 0xfff1;
pub const SHN_COMMON: u16 = 0xfff2;
pub const SHN_XINDEX: u16 = 0xffff;
pub const SHN_HIRESERVE: u16 = 0xffff;

pub const SHT_LOOS: u32 = 0x60000000;
pub const SHT_HIOS: u32 = 0x6fffffff;
pub const SHT_LOPROC: u32 = 0x70000000;
pub const SHT_HIPROC: u32 = 0x7fffffff;
pub const SHT_LOUSER: u32 = 0x80000000;
pub const SHT_HIUSER: u32 = 0xffffffff;

#[derive(Debug, Copy, Clone)]
pub struct SectionHeader<'a> {
    pub sh_name: Elf64Word,
    pub sh_type: Type,
    pub sh_flags: Flags,
    pub sh_addr: Elf64Addr,
    pub sh_offset: Elf64Off,
    pub sh_size: Elf64Xword,
    pub sh_link: Elf64Word,
    pub sh_info: Elf64Word,
    pub sh_addralign: Elf64Xword,
    pub sh_entsize: Elf64Xword,
    file_data: &'a [u8],
}

#[repr(C)]
struct Elf32Shdr {
    sh_name: Elf32Word,
    sh_type: Elf32Word,
    sh_flags: Elf32Word,
    sh_addr: Elf32Addr,
    sh_offset: Elf32Off,
    sh_size: Elf32Word,
    sh_link: Elf32Word,
    sh_info: Elf32Word,
    sh_addralign: Elf32Word,
    sh_entsize: Elf32Word,
}

#[repr(C)]
struct Elf64Shdr {
    sh_name: Elf64Word,
    sh_type: Elf64Word,
    sh_flags: Elf64Xword,
    sh_addr: Elf64Addr,
    sh_offset: Elf64Off,
    sh_size: Elf64Xword,
    sh_link: Elf64Word,
    sh_info: Elf64Word,
    sh_addralign: Elf64Xword,
    sh_entsize: Elf64Xword,
}

#[derive(Debug, Copy, Clone)]
pub enum Type {
    Null,
    ProgBits,
    SymTab,
    StrTab,
    Rela,
    Hash,
    Dynamic,
    Note,
    NoBits,
    Rel,
    ShLib,
    DynSym,
    InitArray,
    FiniArray,
    PreInitArray,
    Group,
    SymTabShndx,
    OsSpecific(u32),
    ProcessorSpecific(ProcessorSpecificType),
    UserApplication(u32),
}

#[derive(Debug, Copy, Clone)]
pub enum ProcessorSpecificType {
    ARMType(ARMType),
    Other(u32),
}

impl Display for ProcessorSpecificType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ARMType(arm_type) => write!(f, "{}", arm_type),
            Self::Other(other) => write!(f, "PROCESSOR SPECIFIC ({})", other),
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct ARMType(u32);

impl ARMType {
    const SHT_ARM_EXIDX: u32 = SHT_LOPROC + 1;
    const SHT_ARM_PREEMPTMAP: u32 = SHT_LOPROC + 2;
    const SHT_ARM_ATTRIBUTES: u32 = SHT_LOPROC + 3;

    pub fn unwind_section(&self) -> bool {
        self.0 == Self::SHT_ARM_EXIDX
    }

    pub fn preempt_map(&self) -> bool {
        self.0 == Self::SHT_ARM_PREEMPTMAP
    }

    pub fn attributes(&self) -> bool {
        self.0 == Self::SHT_ARM_ATTRIBUTES
    }
}

impl From<Elf32Word> for ARMType {
    fn from(value: Elf32Word) -> Self {
        Self(value)
    }
}

impl Display for ARMType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            Self::SHT_ARM_EXIDX => write!(f, "ARM_EXIDX"),
            Self::SHT_ARM_PREEMPTMAP => write!(f, "ARM_PREEMPTMAP"),
            Self::SHT_ARM_ATTRIBUTES => write!(f, "ARM_ATTRIBUTES"),
            _ => write!(f, "ARM SPECIFIC ({:#x})", self.0),
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::Null => write!(f, "NULL"),
            Self::ProgBits => write!(f, "PROGBITS"),
            Self::SymTab => write!(f, "SYMTAB"),
            Self::StrTab => write!(f, "STRTAB"),
            Self::Rela => write!(f, "RELA"),
            Self::Hash => write!(f, "HASH"),
            Self::Dynamic => write!(f, "DYNAMIC"),
            Self::Note => write!(f, "NOTE"),
            Self::NoBits => write!(f, "NOBITS"),
            Self::Rel => write!(f, "REL"),
            Self::ShLib => write!(f, "SHLIB"),
            Self::DynSym => write!(f, "DYNSYM"),
            Self::InitArray => write!(f, "INIT_ARRAY"),
            Self::FiniArray => write!(f, "FINI_ARRAY"),
            Self::PreInitArray => write!(f, "PREINIT_ARRAY"),
            Self::Group => write!(f, "GROUP"),
            Self::SymTabShndx => write!(f, "SYMTAB SECTION INDICES"),
            Self::OsSpecific(os) => write!(f, "OS SPECIFIC ({:#x})", os),
            Self::ProcessorSpecific(proc) => write!(f, "{}", proc),
            Self::UserApplication(app) => write!(f, "USER APPLICATION ({:#x})", app),
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct Flags(u64);

impl Flags {
    const WRITE: u64 = 0x1;
    const ALLOC: u64 = 0x2;
    const EXECINSTR: u64 = 0x4;
    const MERGE: u64 = 0x10;
    const STRINGS: u64 = 0x20;
    const INFO_LINK: u64 = 0x40;
    const LINK_ORDER: u64 = 0x80;
    const OS_NONCONFORMING: u64 = 0x100;
    const GROUP: u64 = 0x200;
    const TLS: u64 = 0x400;
    const MASKOS: u64 = 0x0ff00000;
    const MASKPROC: u64 = 0xf0000000;

    pub fn write(&self) -> bool {
        self.0 & Self::WRITE != 0
    }
    pub fn alloc(&self) -> bool {
        self.0 & Self::ALLOC != 0
    }
    pub fn execinstr(&self) -> bool {
        self.0 & Self::EXECINSTR != 0
    }
    pub fn merge(&self) -> bool {
        self.0 & Self::MERGE != 0
    }
    pub fn strings(&self) -> bool {
        self.0 & Self::STRINGS != 0
    }
    pub fn info_link(&self) -> bool {
        self.0 & Self::INFO_LINK != 0
    }
    pub fn link_order(&self) -> bool {
        self.0 & Self::LINK_ORDER != 0
    }
    pub fn os_nonconforming(&self) -> bool {
        self.0 & Self::OS_NONCONFORMING != 0
    }
    pub fn group(&self) -> bool {
        self.0 & Self::GROUP != 0
    }
    pub fn tls(&self) -> bool {
        self.0 & Self::TLS != 0
    }
    pub fn maskos(&self) -> u64 {
        self.0 & Self::MASKOS
    }
    pub fn maskproc(&self) -> u64 {
        self.0 & Self::MASKPROC
    }
}

impl From<Elf64Xword> for Flags {
    fn from(flags: Elf64Xword) -> Self {
        Self(flags)
    }
}

impl From<Elf32Word> for Flags {
    fn from(flags: Elf32Word) -> Self {
        Self::from(flags as Elf64Xword)
    }
}

impl Display for Flags {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.write() {
            write!(f, "W")?;
        }
        if self.alloc() {
            write!(f, "A")?;
        }
        if self.execinstr() {
            write!(f, "X")?;
        }
        if self.merge() {
            write!(f, "M")?;
        }
        if self.strings() {
            write!(f, "S")?;
        }
        if self.info_link() {
            write!(f, "I")?;
        }
        if self.link_order() {
            write!(f, "L")?;
        }
        if self.os_nonconforming() {
            write!(f, "O")?;
        }
        if self.group() {
            write!(f, "G")?;
        }
        if self.tls() {
            write!(f, "T")?;
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SectionHeaderError {
    InvalidLength,
    InvalidIndex,
    UnknownType,
}

impl error::Error for SectionHeaderError {}

impl Display for SectionHeaderError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::InvalidLength => write!(f, "Invalid section header length"),
            Self::InvalidIndex => write!(f, "Invalid section header index"),
            Self::UnknownType => write!(f, "Unknown section header type"),
        }
    }
}

impl<'a> SectionHeader<'a> {
    pub(crate) fn new(elf: &'a Elf, offset: usize) -> Result<Self, SectionHeaderError> {
        match elf.identity().class {
            identity::Class::ELF32 => {
                Self::new_shdr32(elf.file_data, offset, elf.elf_header().e_machine())
            }
            identity::Class::ELF64 => {
                Self::new_shdr64(elf.file_data, offset, elf.elf_header().e_machine())
            }
        }
    }

    pub fn name(&self, section_string_table_header: &SectionHeader) -> Result<&'a str, Utf8Error> {
        let string_table_offset = section_string_table_header.sh_offset as usize;
        let index = self.sh_name as usize;
        let byte_offset = string_table_offset + index;
        let mut end = byte_offset;
        while self.file_data[end] != 0 {
            end += 1;
        }
        let name = &self.file_data[byte_offset..end];
        core::str::from_utf8(name)
    }

    fn new_shdr32(
        file_data: &'a [u8],
        offset: usize,
        machine: elf_header::Machine,
    ) -> Result<Self, SectionHeaderError> {
        if file_data.len() < offset + size_of::<Elf32Shdr>() {
            return Err(SectionHeaderError::InvalidLength);
        }
        let data = &file_data[offset..offset + size_of::<Elf32Shdr>()];
        let header: &Elf32Shdr = unsafe { &*(data.as_ptr() as *const Elf32Shdr) };

        let sh_name = header.sh_name;
        let sh_type = match header.sh_type {
            0 => Type::Null,
            1 => Type::ProgBits,
            2 => Type::SymTab,
            3 => Type::StrTab,
            4 => Type::Rela,
            5 => Type::Hash,
            6 => Type::Dynamic,
            7 => Type::Note,
            8 => Type::NoBits,
            9 => Type::Rel,
            10 => Type::ShLib,
            11 => Type::DynSym,
            14 => Type::InitArray,
            15 => Type::FiniArray,
            16 => Type::PreInitArray,
            17 => Type::Group,
            18 => Type::SymTabShndx,
            other if other >= SHT_LOOS && other <= SHT_HIOS => Type::OsSpecific(other),
            other if other >= SHT_LOPROC && other <= SHT_HIPROC => match machine {
                elf_header::Machine::ARM => {
                    Type::ProcessorSpecific(ProcessorSpecificType::ARMType(ARMType::from(other)))
                }
                _ => Type::ProcessorSpecific(ProcessorSpecificType::Other(other)),
            },
            other if other >= SHT_LOUSER && other <= SHT_HIUSER => Type::UserApplication(other),
            _ => return Err(SectionHeaderError::UnknownType),
        };
        let sh_flags = Flags::from(header.sh_flags as u64);
        let sh_addr = header.sh_addr as u64;
        let sh_offset = header.sh_offset as u64;
        let sh_size = header.sh_size as u64;
        let sh_link = header.sh_link;
        let sh_info = header.sh_info;
        let sh_addralign = header.sh_addralign as u64;
        let sh_entsize = header.sh_entsize as u64;

        Ok(Self {
            sh_name,
            sh_type,
            sh_flags,
            sh_addr,
            sh_offset,
            sh_size,
            sh_link,
            sh_info,
            sh_addralign,
            sh_entsize,
            file_data,
        })
    }
    fn new_shdr64(
        file_data: &'a [u8],
        offset: usize,
        machine: elf_header::Machine,
    ) -> Result<Self, SectionHeaderError> {
        if file_data.len() < offset + size_of::<Elf64Shdr>() {
            return Err(SectionHeaderError::InvalidLength);
        }
        let data = &file_data[offset..offset + size_of::<Elf64Shdr>()];
        let header: &Elf64Shdr = unsafe { &*(data.as_ptr() as *const Elf64Shdr) };

        let sh_name = header.sh_name;
        let sh_type = match header.sh_type {
            0 => Type::Null,
            1 => Type::ProgBits,
            2 => Type::SymTab,
            3 => Type::StrTab,
            4 => Type::Rela,
            5 => Type::Hash,
            6 => Type::Dynamic,
            7 => Type::Note,
            8 => Type::NoBits,
            9 => Type::Rel,
            10 => Type::ShLib,
            11 => Type::DynSym,
            14 => Type::InitArray,
            15 => Type::FiniArray,
            16 => Type::PreInitArray,
            17 => Type::Group,
            18 => Type::SymTabShndx,
            other if other >= SHT_LOOS && other <= SHT_HIOS => Type::OsSpecific(other),
            other if other >= SHT_LOPROC && other <= SHT_HIPROC => match machine {
                elf_header::Machine::ARM => {
                    Type::ProcessorSpecific(ProcessorSpecificType::ARMType(ARMType::from(other)))
                }
                _ => Type::ProcessorSpecific(ProcessorSpecificType::Other(other)),
            },
            other if other >= SHT_LOUSER && other <= SHT_HIUSER => Type::UserApplication(other),
            _ => return Err(SectionHeaderError::UnknownType),
        };
        let sh_flags = Flags::from(header.sh_flags);
        let sh_addr = header.sh_addr;
        let sh_offset = header.sh_offset;
        let sh_size = header.sh_size;
        let sh_link = header.sh_link;
        let sh_info = header.sh_info;
        let sh_addralign = header.sh_addralign;
        let sh_entsize = header.sh_entsize;

        Ok(Self {
            sh_name,
            sh_type,
            sh_flags,
            sh_addr,
            sh_offset,
            sh_size,
            sh_link,
            sh_info,
            sh_addralign,
            sh_entsize,
            file_data,
        })
    }
}
