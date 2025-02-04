// https://refspecs.linuxfoundation.org/elf/gabi4+/ch5.pheader.html

use core::{
    error::Error,
    fmt::{Display, Formatter},
};

use super::{elf_header, identity, types::*, Elf};

const PT_LOOS: u32 = 0x60000000;
const PT_HIOS: u32 = 0x6fffffff;
const PT_LOPROC: u32 = 0x70000000;
const PT_HIPROC: u32 = 0x7fffffff;

#[derive(Debug, Copy, Clone)]
pub struct ProgramHeader {
    pub p_type: Type,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_flags: Flags,
    pub p_align: u64,
}

#[repr(C)]
struct Elf32Phdr {
    p_type: Elf32Word,
    p_offset: Elf32Off,
    p_vaddr: Elf32Addr,
    p_paddr: Elf32Addr,
    p_filesz: Elf32Word,
    p_memsz: Elf32Word,
    p_flags: Elf32Word,
    p_align: Elf32Word,
}

#[repr(C)]
struct Elf64Phdr {
    p_type: Elf64Word,
    p_flags: Elf64Word,
    p_offset: Elf64Off,
    p_vaddr: Elf64Addr,
    p_paddr: Elf64Addr,
    p_filesz: Elf64Xword,
    p_memsz: Elf64Xword,
    p_align: Elf64Xword,
}

#[derive(Debug, Copy, Clone)]
pub enum GnuType {
    EhFrame,
    Stack,
    Relro,
    Property,
    SFrame,
}

#[derive(Debug, Copy, Clone)]
pub enum SunType {
    Bss,
    Stack,
}

#[derive(Debug, Copy, Clone)]
pub enum OsSpecificType {
    GNU(GnuType),
    Sun(SunType),
    Unknown(u32),
}

impl Display for OsSpecificType {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            OsSpecificType::GNU(gnu) => write!(
                f,
                "GNU_{}",
                match gnu {
                    GnuType::EhFrame => "EH_FRAME",
                    GnuType::Stack => "STACK",
                    GnuType::Relro => "RELRO",
                    GnuType::Property => "PROPERTY",
                    GnuType::SFrame => "SFRAME",
                }
            ),
            OsSpecificType::Sun(sun) => write!(
                f,
                "SUNW{}",
                match sun {
                    SunType::Bss => "BSS",
                    SunType::Stack => "STACK",
                }
            ),
            OsSpecificType::Unknown(other) => write!(f, "OS_SPECIFIC({})", other),
        }
    }
}

impl From<u32> for OsSpecificType {
    fn from(value: u32) -> Self {
        match value {
            0x6474e550 => OsSpecificType::GNU(GnuType::EhFrame),
            0x6474e551 => OsSpecificType::GNU(GnuType::Stack),
            0x6474e552 => OsSpecificType::GNU(GnuType::Relro),
            0x6474e553 => OsSpecificType::GNU(GnuType::Property),
            0x6474e554 => OsSpecificType::GNU(GnuType::SFrame),
            0x6ffffffa => OsSpecificType::Sun(SunType::Bss),
            0x6ffffffb => OsSpecificType::Sun(SunType::Stack),
            other => OsSpecificType::Unknown(other),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ARMType {
    Exidx,
}

#[derive(Debug, Copy, Clone)]
pub enum AArch64Type {
    MemTagMTE,
}

#[derive(Debug, Copy, Clone)]
pub enum ProcessorSpecificType {
    ARM(ARMType),
    AArch64(AArch64Type),
    Unknown(u32),
}

impl ProcessorSpecificType {
    fn new(value: u32, machine: elf_header::Machine) -> Self {
        match value {
            val if matches!(machine, elf_header::Machine::ARM) && val == PT_LOPROC + 1 => {
                ProcessorSpecificType::ARM(ARMType::Exidx)
            }
            val if matches!(machine, elf_header::Machine::AArch64) && val == PT_LOPROC + 2 => {
                ProcessorSpecificType::AArch64(AArch64Type::MemTagMTE)
            }
            other => ProcessorSpecificType::Unknown(other),
        }
    }
}

impl Display for ProcessorSpecificType {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            ProcessorSpecificType::ARM(arm) => write!(
                f,
                "ARM_{}",
                match arm {
                    ARMType::Exidx => "EXIDX",
                }
            ),
            ProcessorSpecificType::AArch64(aarch64) => write!(
                f,
                "AArch64_{}",
                match aarch64 {
                    AArch64Type::MemTagMTE => "MEMTAG_MTE",
                }
            ),
            ProcessorSpecificType::Unknown(other) => write!(f, "PROCESSOR_SPECIFIC({})", other),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Type {
    Null,
    Load,
    Dynamic,
    Interp,
    Note,
    Shlib,
    Phdr,
    Tls,
    OsSpecific(OsSpecificType),
    ProcessorSpecific(ProcessorSpecificType),
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            Self::Null => write!(f, "NULL"),
            Self::Load => write!(f, "LOAD"),
            Self::Dynamic => write!(f, "DYNAMIC"),
            Self::Interp => write!(f, "INTERP"),
            Self::Note => write!(f, "NOTE"),
            Self::Shlib => write!(f, "SHLIB"),
            Self::Phdr => write!(f, "PHDR"),
            Self::Tls => write!(f, "TLS"),
            Self::OsSpecific(os) => write!(f, "{}", os),
            Self::ProcessorSpecific(proc) => write!(f, "{}", proc),
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct Flags(u64);

impl Flags {
    const PF_X: u64 = 0x1;
    const PF_W: u64 = 0x2;
    const PF_R: u64 = 0x4;
    const PF_MASKOS: u64 = 0x0ff00000;
    const PF_MASKPROC: u64 = 0xf0000000;

    pub fn execute(&self) -> bool {
        self.0 & Self::PF_X != 0
    }
    pub fn write(&self) -> bool {
        self.0 & Self::PF_W != 0
    }
    pub fn read(&self) -> bool {
        self.0 & Self::PF_R != 0
    }
    pub fn maskos(&self) -> u64 {
        self.0 & Self::PF_MASKOS
    }
    pub fn maskproc(&self) -> u64 {
        self.0 & Self::PF_MASKPROC
    }
}

impl Display for Flags {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        if self.read() {
            write!(f, "R")?;
        }
        if self.write() {
            write!(f, "W")?;
        }
        if self.execute() {
            write!(f, "E")?;
        }
        Ok(())
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

#[derive(Debug)]
pub enum ProgramHeaderError {
    InvalidLength,
    UnknownType,
}

impl Display for ProgramHeaderError {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            Self::InvalidLength => write!(f, "Invalid length"),
            Self::UnknownType => write!(f, "Unknown type"),
        }
    }
}

impl Error for ProgramHeaderError {}

impl<'a> ProgramHeader {
    pub(crate) fn new(elf: &'a Elf, offset: usize) -> Result<Self, ProgramHeaderError> {
        match elf.identity().class {
            identity::Class::ELF32 => {
                Self::new_phdr32(elf.file_data, offset, elf.elf_header.e_machine())
            }
            identity::Class::ELF64 => {
                Self::new_phdr64(elf.file_data, offset, elf.elf_header.e_machine())
            }
        }
    }

    fn new_phdr32(
        file_data: &'a [u8],
        offset: usize,
        machine: elf_header::Machine,
    ) -> Result<Self, ProgramHeaderError> {
        if file_data.len() < offset + size_of::<Elf32Phdr>() {
            return Err(ProgramHeaderError::InvalidLength);
        }
        let data = &file_data[offset..offset + size_of::<Elf32Phdr>()];
        let header: &Elf32Phdr = unsafe { &*(data.as_ptr() as *const Elf32Phdr) };

        let p_type = match header.p_type {
            0 => Type::Null,
            1 => Type::Load,
            2 => Type::Dynamic,
            3 => Type::Interp,
            4 => Type::Note,
            5 => Type::Shlib,
            6 => Type::Phdr,
            7 => Type::Tls,
            other if other >= PT_LOOS && other <= PT_HIOS => {
                Type::OsSpecific(OsSpecificType::from(other))
            }
            other if other >= PT_LOPROC && other <= PT_HIPROC => {
                Type::ProcessorSpecific(ProcessorSpecificType::new(other, machine))
            }
            _ => return Err(ProgramHeaderError::UnknownType),
        };
        let p_offset = header.p_offset as u64;
        let p_vaddr = header.p_vaddr as u64;
        let p_paddr = header.p_paddr as u64;
        let p_filesz = header.p_filesz as u64;
        let p_memsz = header.p_memsz as u64;
        let p_flags = Flags::from(header.p_flags);
        let p_align = header.p_align as u64;

        Ok(Self {
            p_type,
            p_offset,
            p_vaddr,
            p_paddr,
            p_filesz,
            p_memsz,
            p_flags,
            p_align,
        })
    }
    fn new_phdr64(
        file_data: &'a [u8],
        offset: usize,
        machine: elf_header::Machine,
    ) -> Result<Self, ProgramHeaderError> {
        if file_data.len() < offset + size_of::<Elf64Phdr>() {
            return Err(ProgramHeaderError::InvalidLength);
        }
        let data = &file_data[offset..offset + size_of::<Elf64Phdr>()];
        let header: &Elf64Phdr = unsafe { &*(data.as_ptr() as *const Elf64Phdr) };

        let p_type = match header.p_type {
            0 => Type::Null,
            1 => Type::Load,
            2 => Type::Dynamic,
            3 => Type::Interp,
            4 => Type::Note,
            5 => Type::Shlib,
            6 => Type::Phdr,
            7 => Type::Tls,
            other if other >= PT_LOOS && other <= PT_HIOS => {
                Type::OsSpecific(OsSpecificType::from(other))
            }
            other if other >= PT_LOPROC && other <= PT_HIPROC => {
                Type::ProcessorSpecific(ProcessorSpecificType::new(other, machine))
            }
            _ => return Err(ProgramHeaderError::UnknownType),
        };
        let p_offset = header.p_offset;
        let p_vaddr = header.p_vaddr;
        let p_paddr = header.p_paddr;
        let p_filesz = header.p_filesz;
        let p_memsz = header.p_memsz;
        let p_flags = Flags::from(header.p_flags);
        let p_align = header.p_align;

        Ok(Self {
            p_type,
            p_offset,
            p_vaddr,
            p_paddr,
            p_filesz,
            p_memsz,
            p_flags,
            p_align,
        })
    }
}
