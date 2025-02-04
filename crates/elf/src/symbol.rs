// https://refspecs.linuxfoundation.org/elf/gabi4+/ch4.symtab.html

use core::{
    error,
    fmt::{Display, Formatter},
    str::Utf8Error,
};

use super::{identity, section_header, types::*, Elf};

pub const STN_UNDEF: u32 = 0;

const STB_LOOS: u8 = 10;
const STB_HIOS: u8 = 12;
const STB_LOPROC: u8 = 13;
const STB_HIPROC: u8 = 15;

const STT_LOOS: u8 = 10;
const STT_HIOS: u8 = 12;
const STT_LOPROC: u8 = 13;
const STT_HIPROC: u8 = 15;

#[derive(Debug, Copy, Clone)]
pub struct Symbol<'a> {
    pub st_name: u32,
    pub st_type: Type,
    pub st_bind: Binding,
    pub st_visibility: Visibility,
    pub st_shndx: section_header::Index,
    pub st_value: u64,
    pub st_size: u64,
    elf: &'a Elf<'a>,
}

#[repr(C)]
struct Elf32Sym {
    st_name: Elf32Word,
    st_value: Elf32Addr,
    st_size: Elf32Word,
    st_info: u8,
    st_other: u8,
    st_shndx: Elf32Half,
}

#[repr(C)]
struct Elf64Sym {
    st_name: Elf64Word,
    st_info: u8,
    st_other: u8,
    st_shndx: Elf64Half,
    st_value: Elf64Addr,
    st_size: Elf64Xword,
}

#[repr(transparent)]
pub struct Info(u8);

impl Info {
    pub fn st_bind(&self) -> Result<Binding, SymbolError> {
        Binding::try_from(self.0 >> 4)
    }

    pub fn st_type(&self) -> Result<Type, SymbolError> {
        Type::try_from(self.0 & 0xf)
    }

    pub fn st_info(binding: u8, symbol_type: u8) -> Self {
        Self(binding << 4 + symbol_type)
    }
}

#[repr(transparent)]
pub struct Other(u8);

impl Other {
    pub fn st_visibility(&self) -> Result<Visibility, SymbolError> {
        Visibility::try_from(self.0 & 0x3)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Binding {
    Local,
    Global,
    Weak,
    OsSpecific(u8),
    ProcessorSpecific(u8),
}

impl Display for Binding {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            Self::Local => write!(f, "LOCAL"),
            Self::Global => write!(f, "GLOBAL"),
            Self::Weak => write!(f, "WEAK"),
            Self::OsSpecific(value) => write!(f, "OS Specific ({})", value),
            Self::ProcessorSpecific(value) => write!(f, "Processor Specific ({})", value),
        }
    }
}

impl TryFrom<u8> for Binding {
    type Error = SymbolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Local),
            1 => Ok(Self::Global),
            2 => Ok(Self::Weak),
            STB_LOOS..=STB_HIOS => Ok(Self::OsSpecific(value)),
            STB_LOPROC..=STB_HIPROC => Ok(Self::ProcessorSpecific(value)),
            value => Err(Self::Error::UnknownBinding(value)),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Type {
    NoType,
    Object,
    Func,
    Section,
    File,
    Common,
    Tls,
    OsSpecific(u8),
    ProcessorSpecific(u8),
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            Self::NoType => write!(f, "NOTYPE"),
            Self::Object => write!(f, "OBJECT"),
            Self::Func => write!(f, "FUNC"),
            Self::Section => write!(f, "SECTION"),
            Self::File => write!(f, "FILE"),
            Self::Common => write!(f, "COMMON"),
            Self::Tls => write!(f, "TLS"),
            Self::OsSpecific(value) => write!(f, "OS Specific ({})", value),
            Self::ProcessorSpecific(value) => write!(f, "Processor Specific ({})", value),
        }
    }
}

impl TryFrom<u8> for Type {
    type Error = SymbolError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NoType),
            1 => Ok(Self::Object),
            2 => Ok(Self::Func),
            3 => Ok(Self::Section),
            4 => Ok(Self::File),
            5 => Ok(Self::Common),
            6 => Ok(Self::Tls),
            STT_LOOS..=STT_HIOS => Ok(Self::OsSpecific(value)),
            STT_LOPROC..=STT_HIPROC => Ok(Self::ProcessorSpecific(value)),
            value => Err(Self::Error::UnknownType(value)),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Visibility {
    Default,
    Internal,
    Hidden,
    Protected,
}

impl Display for Visibility {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            Self::Default => write!(f, "DEFAULT"),
            Self::Internal => write!(f, "INTERNAL"),
            Self::Hidden => write!(f, "HIDDEN"),
            Self::Protected => write!(f, "PROTECTED"),
        }
    }
}

impl TryFrom<u8> for Visibility {
    type Error = SymbolError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Default),
            1 => Ok(Self::Internal),
            2 => Ok(Self::Hidden),
            3 => Ok(Self::Protected),
            _ => Err(Self::Error::UnknownVisibility(value)),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SymbolError {
    InvalidLength,
    UnknownBinding(u8),
    UnknownType(u8),
    UnknownVisibility(u8),
}

impl Display for SymbolError {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            Self::InvalidLength => write!(f, "Invalid length"),
            Self::UnknownBinding(value) => write!(f, "Unknown binding: {}", value),
            Self::UnknownType(value) => write!(f, "Unknown type: {}", value),
            Self::UnknownVisibility(value) => write!(f, "Unknown visibility: {}", value),
        }
    }
}

impl error::Error for SymbolError {}

impl<'a> Symbol<'a> {
    pub(crate) fn new(elf: &'a Elf, offset: usize) -> Result<Self, SymbolError> {
        match elf.identity().class {
            identity::Class::ELF32 => Self::new_32(elf, offset),
            identity::Class::ELF64 => Self::new_64(elf, offset),
        }
    }

    fn new_32(elf: &'a Elf, offset: usize) -> Result<Self, SymbolError> {
        if elf.file_data.len() < offset + size_of::<Elf32Sym>() {
            return Err(SymbolError::InvalidLength);
        }
        let data = &elf.file_data[offset..];
        let header: &Elf32Sym = unsafe { &*(data.as_ptr() as *const Elf32Sym) };

        let st_name = header.st_name;
        let st_info = Info(header.st_info);
        let st_bind = st_info.st_bind()?;
        let st_type = st_info.st_type()?;
        let st_other = Other(header.st_other);
        let st_visibility = st_other.st_visibility()?;
        // TODO proper shndx
        let st_shndx = section_header::Index::from(header.st_shndx);
        let st_value = header.st_value as u64;
        let st_size = header.st_size as u64;
        Ok(Self {
            st_name,
            st_type,
            st_bind,
            st_visibility,
            st_shndx,
            st_value,
            st_size,
            elf,
        })
    }

    fn new_64(elf: &'a Elf, offset: usize) -> Result<Self, SymbolError> {
        if elf.file_data.len() < offset + size_of::<Elf64Sym>() {
            return Err(SymbolError::InvalidLength);
        }
        let data = &elf.file_data[offset..];
        let header: &Elf64Sym = unsafe { &*(data.as_ptr() as *const Elf64Sym) };

        let st_name = header.st_name;
        let st_info = Info(header.st_info);
        let st_bind = st_info.st_bind()?;
        let st_type = st_info.st_type()?;
        let st_other = Other(header.st_other);
        let st_visibility = st_other.st_visibility()?;
        // TODO proper shndx
        let st_shndx = section_header::Index::from(header.st_shndx);
        let st_value = header.st_value;
        let st_size = header.st_size;
        Ok(Self {
            st_name,
            st_type,
            st_bind,
            st_visibility,
            st_shndx,
            st_value,
            st_size,
            elf,
        })
    }

    pub fn name(
        &self,
        string_table_header: &section_header::SectionHeader,
    ) -> Result<&'a str, Utf8Error> {
        let string_table_offset = string_table_header.sh_offset as usize;
        let index = self.st_name as usize;
        let byte_offset = string_table_offset + index;
        let mut end = byte_offset;
        while self.elf.file_data[end] != 0 {
            end += 1;
        }
        let name = &self.elf.file_data[byte_offset..end];
        core::str::from_utf8(name)
    }
}
