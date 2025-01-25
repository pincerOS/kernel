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
