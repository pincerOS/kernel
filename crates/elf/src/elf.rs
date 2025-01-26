use core::{
    error::Error,
    fmt::{self, Display},
};

pub mod elf_header;
pub mod identity;
pub mod program_header;
pub mod section_header;

// /usr/include/elf.h

mod types {
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

pub struct Elf<'a> {
    file_data: &'a [u8],
    elf_header: elf_header::ElfHeader<'a>,
}

#[derive(Debug)]
pub enum ElfError<'a> {
    ElfHeaderError(elf_header::ElfHeaderError<'a>),
    SectionHeaderError(section_header::SectionHeaderError),
    ProgramHeaderError(program_header::ProgramHeaderError),
}

impl Display for ElfError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ElfHeaderError(e) => write!(f, "Error parsing ELF header: {}", e),
            Self::SectionHeaderError(e) => write!(f, "Error parsing section header: {}", e),
            Self::ProgramHeaderError(e) => write!(f, "Error parsing program header: {}", e),
        }
    }
}

impl<'a> From<elf_header::ElfHeaderError<'a>> for ElfError<'a> {
    fn from(e: elf_header::ElfHeaderError<'a>) -> Self {
        Self::ElfHeaderError(e)
    }
}

impl From<section_header::SectionHeaderError> for ElfError<'_> {
    fn from(e: section_header::SectionHeaderError) -> Self {
        Self::SectionHeaderError(e)
    }
}

impl From<program_header::ProgramHeaderError> for ElfError<'_> {
    fn from(e: program_header::ProgramHeaderError) -> Self {
        Self::ProgramHeaderError(e)
    }
}

impl Error for ElfError<'_> {}

impl<'a> Elf<'a> {
    pub fn new(elf_data: &'a [u8]) -> Result<Self, ElfError<'a>> {
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

    pub fn section_headers(
        &'a self,
    ) -> Result<
        impl Iterator<Item = Result<section_header::SectionHeader<'a>, ElfError<'a>>>,
        ElfError<'a>,
    > {
        let table_start = self.elf_header.e_shoff() as usize;
        let entry_size = self.elf_header.e_shentsize() as usize;
        let entry_count = if self.elf_header.e_shnum() == section_header::SHN_UNDEF {
            let first_section_header = section_header::SectionHeader::new(&self, table_start)?;
            first_section_header.sh_size as usize
        } else {
            self.elf_header.e_shnum() as usize
        };

        let iter = (0..entry_count).map(move |i| {
            let entry_start = i * entry_size + table_start;
            match section_header::SectionHeader::new(&self, entry_start) {
                Ok(header) => Ok(header),
                Err(e) => Err(e.into()),
            }
        });
        Ok(iter)
    }

    pub fn program_headers(
        &'a self,
    ) -> Result<
        impl Iterator<Item = Result<program_header::ProgramHeader<'a>, ElfError<'a>>>,
        ElfError<'a>,
    > {
        let table_start = self.elf_header.e_phoff() as usize;
        let entry_size = self.elf_header.e_phentsize() as usize;
        let entry_count = self.elf_header.e_phnum() as usize;

        let iter = (0..entry_count).map(move |i| {
            let entry_start = i * entry_size + table_start;
            match program_header::ProgramHeader::new(&self, entry_start) {
                Ok(header) => Ok(header),
                Err(e) => Err(e.into()),
            }
        });
        Ok(iter)
    }
}
