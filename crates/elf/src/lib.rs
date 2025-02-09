#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use core::{
    error::Error,
    fmt::{self, Display},
};

pub mod elf_header;
pub mod identity;
pub mod program_header;
pub mod relocation;
pub mod section_header;
pub mod symbol;

// /usr/include/elf.h

pub mod types {
    // https://refspecs.linuxfoundation.org/elf/gabi4+/ch4.intro.html#data_representation
    pub type Elf32Addr = u32;
    pub type Elf32Off = u32;
    pub type Elf32Half = u16;
    pub type Elf32Word = u32;
    pub type Elf32Sword = i32;

    pub type Elf64Addr = u64;
    pub type Elf64Off = u64;
    pub type Elf64Half = u16;
    pub type Elf64Word = u32;
    pub type Elf64Sword = i32;
    pub type Elf64Xword = u64;
    pub type Elf64Sxword = i64;
}

#[derive(Debug)]
pub struct Elf<'a> {
    file_data: &'a [u8],
    elf_header: elf_header::ElfHeader,
}

#[derive(Debug)]
pub enum ElfError {
    ElfHeaderError(elf_header::ElfHeaderError),
    SectionHeaderError(section_header::SectionHeaderError),
    ProgramHeaderError(program_header::ProgramHeaderError),
    RelocationError(relocation::RelocationError),
    SymbolError(symbol::SymbolError),
}

impl Display for ElfError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ElfHeaderError(e) => write!(f, "Error parsing ELF header: {}", e),
            Self::SectionHeaderError(e) => write!(f, "Error parsing section header: {}", e),
            Self::ProgramHeaderError(e) => write!(f, "Error parsing program header: {}", e),
            Self::RelocationError(e) => write!(f, "Error parsing relocation: {}", e),
            Self::SymbolError(e) => write!(f, "Error parsing symbol: {}", e),
        }
    }
}

impl From<elf_header::ElfHeaderError> for ElfError {
    fn from(e: elf_header::ElfHeaderError) -> Self {
        Self::ElfHeaderError(e)
    }
}

impl From<section_header::SectionHeaderError> for ElfError {
    fn from(e: section_header::SectionHeaderError) -> Self {
        Self::SectionHeaderError(e)
    }
}

impl From<program_header::ProgramHeaderError> for ElfError {
    fn from(e: program_header::ProgramHeaderError) -> Self {
        Self::ProgramHeaderError(e)
    }
}

impl From<relocation::RelocationError> for ElfError {
    fn from(e: relocation::RelocationError) -> Self {
        Self::RelocationError(e)
    }
}

impl From<symbol::SymbolError> for ElfError {
    fn from(e: symbol::SymbolError) -> Self {
        Self::SymbolError(e)
    }
}

impl Error for ElfError {}

impl<'a> Elf<'a> {
    pub fn new(elf_data: &'a [u8]) -> Result<Self, ElfError> {
        let elf_header = elf_header::ElfHeader::new(elf_data)?;
        Ok(Self {
            file_data: elf_data,
            elf_header,
        })
    }

    pub fn elf_header(&self) -> &elf_header::ElfHeader {
        &self.elf_header
    }

    pub fn identity(&self) -> identity::ElfIdentity {
        self.elf_header.e_ident()
    }

    pub fn identity_bytes(&self) -> &'a [u8; identity::EI_NIDENT] {
        self.file_data.first_chunk().unwrap()
    }

    pub fn section_headers(
        &'a self,
    ) -> Result<impl Iterator<Item = Result<section_header::SectionHeader<'a>, ElfError>>, ElfError>
    {
        let table_start = self.elf_header.e_shoff() as usize;
        let entry_size = self.elf_header.e_shentsize() as usize;
        let entry_count = if self.elf_header.e_shnum() == section_header::SHN_UNDEF {
            let first_section_header = section_header::SectionHeader::new(self, table_start)?;
            first_section_header.sh_size as usize
        } else {
            self.elf_header.e_shnum() as usize
        };
        let table_end = table_start + entry_size * entry_count;

        let iter = (table_start..table_end)
            .step_by(entry_size)
            .map(
                move |offset| match section_header::SectionHeader::new(self, offset) {
                    Ok(header) => Ok(header),
                    Err(e) => Err(e.into()),
                },
            );
        Ok(iter)
    }

    pub fn symtab_header(&'a self) -> Result<Option<section_header::SectionHeader<'a>>, ElfError> {
        let mut symtab_header = None;
        for header in self.section_headers()? {
            let header = header?;
            if matches!(header.sh_type, section_header::Type::SymTab) {
                symtab_header = Some(header);
                break;
            }
        }
        Ok(symtab_header)
    }

    pub fn dynsym_header(&'a self) -> Result<Option<section_header::SectionHeader<'a>>, ElfError> {
        let mut dynsym_header = None;
        for header in self.section_headers()? {
            let header = header?;
            if matches!(header.sh_type, section_header::Type::DynSym) {
                dynsym_header = Some(header);
                break;
            }
        }
        Ok(dynsym_header)
    }

    pub fn program_headers(
        &'a self,
    ) -> Option<impl Iterator<Item = Result<program_header::ProgramHeader, ElfError>> + 'a> {
        let table_start = self.elf_header.e_phoff() as usize;
        if table_start == 0 {
            return None;
        }
        let entry_size = self.elf_header.e_phentsize() as usize;
        let entry_count = self.elf_header.e_phnum() as usize;
        let table_end = table_start + entry_size * entry_count;

        let iter = (table_start..table_end)
            .step_by(entry_size)
            .map(
                move |offset| match program_header::ProgramHeader::new(self, offset) {
                    Ok(header) => Ok(header),
                    Err(e) => Err(e.into()),
                },
            );
        Some(iter)
    }

    pub fn segment_data(&self, phdr: &program_header::ProgramHeader) -> Option<&'a [u8]> {
        let start = usize::try_from(phdr.p_offset).ok()?;
        let end = usize::try_from(phdr.p_offset.checked_add(phdr.p_filesz)?).ok()?;
        self.file_data.get(start..end)
    }
}
