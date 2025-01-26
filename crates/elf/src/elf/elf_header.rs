// https://refspecs.linuxfoundation.org/elf/gabi4+/ch4.eheader.html

use core::fmt::{self, Display};

use super::identity;
use super::types::*;

// OS specific ELF types
const ET_LOOS: u16 = 0xfe00;
const ET_HIOS: u16 = 0xfeff;
// Processor-specific ELF types
const ET_LOPROC: u16 = 0xff00;
const ET_HIPROC: u16 = 0xffff;

#[derive(Debug, Copy, Clone)]
pub enum ElfHeader<'a> {
    Elf32Header {
        e_ident: identity::ElfIdentity<'a>,
        e_type: Type,
        e_machine: Machine,
        e_version: Version,
        e_entry: Elf32Addr,
        e_phoff: Elf32Off,
        e_shoff: Elf32Off,
        e_flags: Flags,
        e_ehsize: u16,
        e_phentsize: u16,
        e_phnum: u16,
        e_shentsize: u16,
        e_shnum: u16,
        e_shstrndx: u16,
        data: &'a [u8],
    },
    Elf64Header {
        e_ident: identity::ElfIdentity<'a>,
        e_type: Type,
        e_machine: Machine,
        e_version: Version,
        e_entry: Elf64Addr,
        e_phoff: Elf64Off,
        e_shoff: Elf64Off,
        e_flags: Flags,
        e_ehsize: u16,
        e_phentsize: u16,
        e_phnum: u16,
        e_shentsize: u16,
        e_shnum: u16,
        e_shstrndx: u16,
        data: &'a [u8],
    },
}

#[repr(C)]
struct Elf32Ehdr {
    e_ident: [u8; identity::EI_NIDENT],
    e_type: Elf32Half,
    e_machine: Elf32Half,
    e_version: Elf32Word,
    e_entry: Elf32Addr,
    e_phoff: Elf32Off,
    e_shoff: Elf32Off,
    e_flags: Elf32Word,
    e_ehsize: Elf32Half,
    e_phentsize: Elf32Half,
    e_phnum: Elf32Half,
    e_shentsize: Elf32Half,
    e_shnum: Elf32Half,
    e_shstrndx: Elf32Half,
}

#[repr(C)]
struct Elf64Ehdr {
    e_ident: [u8; identity::EI_NIDENT],
    e_type: Elf64Half,
    e_machine: Elf64Half,
    e_version: Elf64Word,
    e_entry: Elf64Addr,
    e_phoff: Elf64Off,
    e_shoff: Elf64Off,
    e_flags: Elf64Word,
    e_ehsize: Elf64Half,
    e_phentsize: Elf64Half,
    e_phnum: Elf64Half,
    e_shentsize: Elf64Half,
    e_shnum: Elf64Half,
    e_shstrndx: Elf64Half,
}

#[derive(Debug, Copy, Clone)]
pub enum Type {
    None,
    Relocatable,
    Executable,
    SharedObject,
    Core,
    OsSpecific(u16),
    ProcessorSpecific(u16),
}

impl Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::None => write!(f, "NONE (None)"),
            Self::Relocatable => write!(f, "REL (Relocatable file)"),
            Self::Executable => write!(f, "EXEC (Executable file)"),
            Self::SharedObject => write!(f, "DYN (Shared object file)"),
            Self::Core => write!(f, "CORE (Core file)"),
            Self::OsSpecific(os) => write!(f, "OS Specific ({})", os),
            Self::ProcessorSpecific(proc) => write!(f, "Processor Specific ({})", proc),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Machine {
    None,
    Bellmac32,
    SPARC,
    I386,
    Motorola68000,
    Motorola88000,
    I80860,
    MIPS,
    IBMSystem370,
    MIPSRS3000,
    PARISC,
    VPP500,
    SPARC32Plus,
    I960,
    PowerPC,
    PowerPC64,
    IBMSystem390,
    NECV800,
    FujitsuFR20,
    TRWRH32,
    MotorolaRCE,
    ARM,
    DigitalAlpha,
    SuperH,
    SPARCV9,
    TriCore,
    ARC,
    H8300,
    H8300H,
    H8S,
    H8500,
    IA64,
    MIPSX,
    ColdFire,
    M68HC12,
    MMA,
    PCP,
    NCPU,
    NDR1,
    StarCore,
    ME16,
    ST100,
    TinyJ,
    X86_64,
    PDSP,
    PDP10,
    PDP11,
    FX66,
    ST9Plus,
    ST7,
    Motorola68HC16,
    Motorola68HC11,
    Motorola68HC08,
    Motorola68HC05,
    SVx,
    ST19,
    VAX,
    CRIS,
    JAVELIN,
    FIREPATH,
    ZSP,
    MMIX,
    HUANY,
    Prism,
    AVR,
    FR30,
    D10V,
    D30V,
    V850,
    M32R,
    MN10300,
    MN10200,
    PicoJava,
    OpenRISC,
    ARCA5,
    Xtensa,
    VideoCore,
    TMMGPP,
    NS32K,
    TPC,
    SNP1K,
    ST200,
    AArch64,
    Reserved(u16),
}

impl Display for Machine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Bellmac32 => write!(f, "Bellmac32"),
            Self::SPARC => write!(f, "SPARC"),
            Self::I386 => write!(f, "Intel 80386"),
            Self::Motorola68000 => write!(f, "Motorola 68000"),
            Self::Motorola88000 => write!(f, "Motorola 88000"),
            Self::I80860 => write!(f, "Intel 80860"),
            Self::MIPS => write!(f, "MIPS R3000"),
            Self::IBMSystem370 => write!(f, "IBM System/370"),
            Self::MIPSRS3000 => write!(f, "MIPS RS3000"),
            Self::PARISC => write!(f, "HPPA"),
            Self::VPP500 => write!(f, "Fujitsu VPP500"),
            Self::SPARC32Plus => write!(f, "Sun's \"v8plus\""),
            Self::I960 => write!(f, "Intel 80960"),
            Self::PowerPC => write!(f, "PowerPC"),
            Self::PowerPC64 => write!(f, "PowerPC 64-bit"),
            Self::IBMSystem390 => write!(f, "IBM System/390"),
            Self::NECV800 => write!(f, "NEC V800 series"),
            Self::FujitsuFR20 => write!(f, "Fujitsu FR20"),
            Self::TRWRH32 => write!(f, "TRW RH-32"),
            Self::MotorolaRCE => write!(f, "Motorola RCE"),
            Self::ARM => write!(f, "ARM"),
            Self::DigitalAlpha => write!(f, "Digital Alpha"),
            Self::SuperH => write!(f, "Hitachi SH"),
            Self::SPARCV9 => write!(f, "SPARC v9 64-bit"),
            Self::TriCore => write!(f, "Siemens TriCore embedded processor"),
            Self::ARC => write!(f, "Argonaut RISC Core, Argonaut Technologies Inc."),
            Self::H8300 => write!(f, "Hitachi H8/300"),
            Self::H8300H => write!(f, "Hitachi H8/300H"),
            Self::H8S => write!(f, "Hitachi H8S"),
            Self::H8500 => write!(f, "Hitachi H8/500"),
            Self::IA64 => write!(f, "Intel IA-64 processor architecture"),
            Self::MIPSX => write!(f, "Stanford MIPS-X"),
            Self::ColdFire => write!(f, "Motorola ColdFire"),
            Self::M68HC12 => write!(f, "Motorola M68HC12"),
            Self::MMA => write!(f, "Fujitsu MMA Multimedia Accelerator"),
            Self::PCP => write!(f, "Siemens PCP"),
            Self::NCPU => write!(f, "Sony nCPU embedded RISC processor"),
            Self::NDR1 => write!(f, "Denso NDR1 microprocessor"),
            Self::StarCore => write!(f, "Motorola Star*Core processor"),
            Self::ME16 => write!(f, "Toyota ME16 processor"),
            Self::ST100 => write!(f, "STMicroelectronics ST100 processor"),
            Self::TinyJ => write!(f, "Advanced Logic Corp. TinyJ embedded processor family"),
            Self::X86_64 => write!(f, "Advanced Micro Devices X86-64"),
            Self::PDSP => write!(f, "Sony DSP Processor"),
            Self::PDP10 => write!(f, "Digital Equipment Corp. PDP-10"),
            Self::PDP11 => write!(f, "Digital Equipment Corp. PDP-11"),
            Self::FX66 => write!(f, "Siemens FX66 microcontroller"),
            Self::ST9Plus => write!(f, "STMicroelectronics ST9+ 8/16 bit microcontroller"),
            Self::ST7 => write!(f, "STMicroelectronics ST7 8-bit microcontroller"),
            Self::Motorola68HC16 => write!(f, "Motorola MC68HC16 Microcontroller"),
            Self::Motorola68HC11 => write!(f, "Motorola MC68HC11 Microcontroller"),
            Self::Motorola68HC08 => write!(f, "Motorola MC68HC08 Microcontroller"),
            Self::Motorola68HC05 => write!(f, "Motorola MC68HC05 Microcontroller"),
            Self::SVx => write!(f, "Silicon Graphics SVx"),
            Self::ST19 => write!(f, "STMicroelectronics ST19 8-bit microcontroller"),
            Self::VAX => write!(f, "Digital VAX"),
            Self::CRIS => write!(f, "Axis Communications 32-bit embedded processor"),
            Self::JAVELIN => write!(f, "Infineon Technologies 32-bit embedded processor"),
            Self::FIREPATH => write!(f, "Element 14 64-bit DSP Processor"),
            Self::ZSP => write!(f, "LSI Logic 16-bit DSP Processor"),
            Self::MMIX => write!(f, "Donald Knuth's educational 64-bit processor"),
            Self::HUANY => write!(f, "Harvard University machine-independent object files"),
            Self::Prism => write!(f, "SiTera Prism"),
            Self::AVR => write!(f, "Atmel AVR 8-bit microcontroller"),
            Self::FR30 => write!(f, "Fujitsu FR30"),
            Self::D10V => write!(f, "Mitsubishi D10V"),
            Self::D30V => write!(f, "Mitsubishi D30V"),
            Self::V850 => write!(f, "NEC v850"),
            Self::M32R => write!(f, "Mitsubishi M32R"),
            Self::MN10300 => write!(f, "Matsushita MN10300"),
            Self::MN10200 => write!(f, "Matsushita MN10200"),
            Self::PicoJava => write!(f, "PicoJava"),
            Self::OpenRISC => write!(f, "OpenRISC 32-bit embedded processor"),
            Self::ARCA5 => write!(f, "ARC Cores Tangent-A5"),
            Self::Xtensa => write!(f, "Tensilica Xtensa Architecture"),
            Self::VideoCore => write!(f, "Alphamosaic VideoCore processor"),
            Self::TMMGPP => write!(f, "Thompson Multimedia General Purpose Processor"),
            Self::NS32K => write!(f, "National Semiconductor 32000 series"),
            Self::TPC => write!(f, "Tenor Network TPC processor"),
            Self::SNP1K => write!(f, "Trebia SNP 1000 processor"),
            Self::ST200 => write!(f, "STMicroelectronics (www.st.com) ST200 microcontroller"),
            Self::AArch64 => write!(f, "AArch64"),
            Self::Reserved(other) => write!(f, "Reserved ({})", other),
        }
    }
}

impl From<u16> for Machine {
    fn from(m: u16) -> Self {
        match m {
            0 => Machine::None,
            1 => Machine::Bellmac32,
            2 => Machine::SPARC,
            3 => Machine::I386,
            4 => Machine::Motorola68000,
            // 6 reserved
            5 => Machine::Motorola88000,
            7 => Machine::I80860,
            8 => Machine::MIPS,
            9 => Machine::IBMSystem370,
            10 => Machine::MIPSRS3000,
            // 11-14 reserved
            15 => Machine::PARISC,
            // 16 reserved
            17 => Machine::VPP500,
            18 => Machine::SPARC32Plus,
            19 => Machine::I960,
            20 => Machine::PowerPC,
            21 => Machine::PowerPC64,
            22 => Machine::IBMSystem390,
            // 23-35 reserved
            36 => Machine::NECV800,
            37 => Machine::FujitsuFR20,
            38 => Machine::TRWRH32,
            39 => Machine::MotorolaRCE,
            40 => Machine::ARM,
            41 => Machine::DigitalAlpha,
            42 => Machine::SuperH,
            43 => Machine::SPARCV9,
            44 => Machine::TriCore,
            45 => Machine::ARC,
            46 => Machine::H8300,
            47 => Machine::H8300H,
            48 => Machine::H8S,
            49 => Machine::H8500,
            50 => Machine::IA64,
            51 => Machine::MIPSX,
            52 => Machine::ColdFire,
            53 => Machine::M68HC12,
            54 => Machine::MMA,
            55 => Machine::PCP,
            56 => Machine::NCPU,
            57 => Machine::NDR1,
            58 => Machine::StarCore,
            59 => Machine::ME16,
            60 => Machine::ST100,
            61 => Machine::TinyJ,
            62 => Machine::X86_64,
            63 => Machine::PDSP,
            64 => Machine::PDP10,
            65 => Machine::PDP11,
            66 => Machine::FX66,
            67 => Machine::ST9Plus,
            68 => Machine::ST7,
            69 => Machine::Motorola68HC16,
            70 => Machine::Motorola68HC11,
            71 => Machine::Motorola68HC08,
            72 => Machine::Motorola68HC05,
            73 => Machine::SVx,
            74 => Machine::ST19,
            75 => Machine::VAX,
            76 => Machine::CRIS,
            77 => Machine::JAVELIN,
            78 => Machine::FIREPATH,
            79 => Machine::ZSP,
            80 => Machine::MMIX,
            81 => Machine::HUANY,
            82 => Machine::Prism,
            83 => Machine::AVR,
            84 => Machine::FR30,
            85 => Machine::D10V,
            86 => Machine::D30V,
            87 => Machine::V850,
            88 => Machine::M32R,
            89 => Machine::MN10300,
            90 => Machine::MN10200,
            91 => Machine::PicoJava,
            92 => Machine::OpenRISC,
            93 => Machine::ARCA5,
            94 => Machine::Xtensa,
            95 => Machine::VideoCore,
            96 => Machine::TMMGPP,
            97 => Machine::NS32K,
            98 => Machine::TPC,
            99 => Machine::SNP1K,
            100 => Machine::ST200,
            183 => Machine::AArch64,
            other => Machine::Reserved(other),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Version {
    Current,
}

impl Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Current => write!(f, "0x1"),
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct ARMFlags(u32);

impl ARMFlags {
    const EF_ARM_RELEXEC: u32 = 0x01;
    const EF_ARM_HASENTRY: u32 = 0x02;
    const EF_ARM_INTERWORK: u32 = 0x04;
    const EF_ARM_APCS_26: u32 = 0x08;
    const EF_ARM_APCS_FLOAT: u32 = 0x10;
    const EF_ARM_PIC: u32 = 0x20;
    const EF_ARM_ALIGN8: u32 = 0x40;
    const EF_ARM_NEW_ABI: u32 = 0x80;
    const EF_ARM_OLD_ABI: u32 = 0x100;
    const EF_ARM_SOFT_FLOAT: u32 = 0x200;
    const EF_ARM_ABI_FLOAT_SOFT: u32 = 0x200;
    const EF_ARM_VFP_FLOAT: u32 = 0x400;
    const EF_ARM_ABI_FLOAT_HARD: u32 = 0x400;
    const EF_ARM_MAVERICK_FLOAT: u32 = 0x800;
    const EF_ARM_EABIMASK: u32 = 0xFF000000;

    pub fn relexec(&self) -> bool {
        self.0 & Self::EF_ARM_RELEXEC != 0
    }
    pub fn hasentry(&self) -> bool {
        self.0 & Self::EF_ARM_HASENTRY != 0
    }
    pub fn interwork(&self) -> bool {
        self.0 & Self::EF_ARM_INTERWORK != 0
    }
    pub fn apcs_26(&self) -> bool {
        self.0 & Self::EF_ARM_APCS_26 != 0
    }
    pub fn apcs_float(&self) -> bool {
        self.0 & Self::EF_ARM_APCS_FLOAT != 0
    }
    pub fn pic(&self) -> bool {
        self.0 & Self::EF_ARM_PIC != 0
    }
    pub fn align8(&self) -> bool {
        self.0 & Self::EF_ARM_ALIGN8 != 0
    }
    pub fn new_abi(&self) -> bool {
        self.0 & Self::EF_ARM_NEW_ABI != 0
    }
    pub fn old_abi(&self) -> bool {
        self.0 & Self::EF_ARM_OLD_ABI != 0
    }
    pub fn soft_float(&self) -> bool {
        self.0 & Self::EF_ARM_SOFT_FLOAT != 0
    }
    pub fn abi_float_soft(&self) -> bool {
        self.0 & Self::EF_ARM_ABI_FLOAT_SOFT != 0
    }
    pub fn vfp_float(&self) -> bool {
        self.0 & Self::EF_ARM_VFP_FLOAT != 0
    }
    pub fn abi_float_hard(&self) -> bool {
        self.0 & Self::EF_ARM_ABI_FLOAT_HARD != 0
    }
    pub fn maverick_float(&self) -> bool {
        self.0 & Self::EF_ARM_MAVERICK_FLOAT != 0
    }
    pub fn eabi_version(&self) -> u32 {
        (self.0 & Self::EF_ARM_EABIMASK) >> 24
    }
}

impl Display for ARMFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:08x}", self.0)?;
        if self.0 == 0 {
            return Ok(());
        }
        match self.eabi_version() {
            1 => write!(f, ", Version1 EABI")?,
            2 => write!(f, ", Version2 EABI")?,
            3 => write!(f, ", Version3 EABI")?,
            4 => write!(f, ", Version4 EABI")?,
            5 => write!(f, ", Version5 EABI")?,
            _ => write!(f, ", Unknown EABI version")?,
        }
        if self.relexec() {
            write!(f, ", RELEXEC")?;
        }
        if self.hasentry() {
            write!(f, ", HASENTRY")?;
        }
        if self.interwork() {
            write!(f, ", INTERWORK")?;
        }
        if self.apcs_26() {
            write!(f, ", APCS_26")?;
        }
        if self.apcs_float() {
            write!(f, ", APCS_FLOAT")?;
        }
        if self.pic() {
            write!(f, ", PIC")?;
        }
        if self.align8() {
            write!(f, ", ALIGN8")?;
        }
        if self.new_abi() {
            write!(f, ", NEW_ABI")?;
        }
        if self.old_abi() {
            write!(f, ", OLD_ABI")?;
        }
        if self.abi_float_soft() {
            write!(f, ", soft-float ABI")?;
        }
        if self.abi_float_hard() {
            write!(f, ", hard-float ABI")?;
        }
        if self.maverick_float() {
            write!(f, ", MAVERICK_FLOAT")?;
        }

        Ok(())
    }
}

impl From<Elf32Word> for ARMFlags {
    fn from(flags: Elf32Word) -> Self {
        ARMFlags(flags)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Flags {
    ARM(ARMFlags),
    AArch64,
    I386,
    X86_64,
}

impl Display for Flags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ARM(flags) => write!(f, "{}", flags),
            _ => write!(f, ""),
        }
    }
}

#[derive(Debug)]
pub enum ElfHeaderError {
    InvalidLength,
    ElfIdentityError(identity::ElfIdentityError),
    InvalidType,
    InvalidVersion,
    UnknownVersion,
    UnimplementedArchitecture,
}

impl Display for ElfHeaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidLength => write!(f, "Invalid ELF header length"),
            Self::ElfIdentityError(e) => write!(f, "Invalid ELF identity: {}", e),
            Self::InvalidType => write!(f, "Invalid ELF type"),
            Self::InvalidVersion => write!(f, "Invalid ELF version"),
            Self::UnknownVersion => write!(f, "Unknown ELF version"),
            Self::UnimplementedArchitecture => write!(f, "Unimplemented architecture"),
        }
    }
}

impl From<identity::ElfIdentityError> for ElfHeaderError {
    fn from(e: identity::ElfIdentityError) -> Self {
        ElfHeaderError::ElfIdentityError(e)
    }
}

impl<'a> ElfHeader<'a> {
    pub(crate) fn new(header: &'a [u8]) -> Result<Self, ElfHeaderError> {
        if header.len() < identity::EI_NIDENT {
            return Err(ElfHeaderError::InvalidLength);
        }

        let e_ident = identity::ElfIdentity::new(&header[0..identity::EI_NIDENT])?;
        match e_ident.class {
            identity::Class::ELF32 => Self::new_elf32(header, e_ident),
            identity::Class::ELF64 => Self::new_elf64(header, e_ident),
        }
    }

    fn new_elf32(
        data: &'a [u8],
        e_ident: identity::ElfIdentity<'a>,
    ) -> Result<Self, ElfHeaderError> {
        if data.len() < size_of::<Elf32Ehdr>() {
            return Err(ElfHeaderError::InvalidLength);
        }
        let header: &Elf32Ehdr = unsafe { &*(data.as_ptr() as *const Elf32Ehdr) };

        let e_type = match header.e_type {
            0x00 => Type::None,
            0x01 => Type::Relocatable,
            0x02 => Type::Executable,
            0x03 => Type::SharedObject,
            0x04 => Type::Core,
            other if other >= ET_LOOS && other <= ET_HIOS => Type::OsSpecific(other),
            other if other >= ET_LOPROC && other <= ET_HIPROC => Type::ProcessorSpecific(other),
            _ => return Err(ElfHeaderError::InvalidType),
        };
        let e_machine = match Machine::from(header.e_machine) {
            Machine::ARM => Machine::ARM,
            Machine::I386 => Machine::I386,
            _ => return Err(ElfHeaderError::UnimplementedArchitecture),
        };
        let e_version = match header.e_version {
            0x00 => return Err(ElfHeaderError::InvalidVersion),
            0x01 => Version::Current,
            _ => return Err(ElfHeaderError::UnknownVersion),
        };
        let e_entry = header.e_entry;
        let e_phoff = header.e_phoff;
        let e_shoff = header.e_shoff;
        let e_flags = match e_machine {
            Machine::ARM => Flags::ARM(ARMFlags::from(header.e_flags)),
            Machine::I386 => Flags::I386,
            _ => return Err(ElfHeaderError::UnimplementedArchitecture),
        };
        let e_ehsize = header.e_ehsize;
        let e_phentsize = header.e_phentsize;
        let e_phnum = header.e_phnum;
        let e_shentsize = header.e_shentsize;
        let e_shnum = header.e_shnum;
        let e_shstrndx = header.e_shstrndx;

        Ok(Self::Elf32Header {
            e_ident,
            e_type,
            e_machine,
            e_version,
            e_entry,
            e_phoff,
            e_shoff,
            e_flags,
            e_ehsize,
            e_phentsize,
            e_phnum,
            e_shentsize,
            e_shnum,
            e_shstrndx,
            data,
        })
    }
    fn new_elf64(
        data: &'a [u8],
        e_ident: identity::ElfIdentity<'a>,
    ) -> Result<Self, ElfHeaderError> {
        if data.len() < size_of::<Elf64Ehdr>() {
            return Err(ElfHeaderError::InvalidLength);
        }
        let header: &Elf64Ehdr = unsafe { &*(data.as_ptr() as *const Elf64Ehdr) };

        let e_type = match header.e_type {
            0x00 => Type::None,
            0x01 => Type::Relocatable,
            0x02 => Type::Executable,
            0x03 => Type::SharedObject,
            0x04 => Type::Core,
            other if other >= ET_LOOS && other <= ET_HIOS => Type::OsSpecific(other),
            other if other >= ET_LOPROC && other <= ET_HIPROC => Type::ProcessorSpecific(other),
            _ => return Err(ElfHeaderError::InvalidType),
        };
        let e_machine = Machine::from(header.e_machine);
        let e_version = match header.e_version {
            0x00 => return Err(ElfHeaderError::InvalidVersion),
            0x01 => Version::Current,
            _ => return Err(ElfHeaderError::UnknownVersion),
        };
        let e_entry = header.e_entry;
        let e_phoff = header.e_phoff;
        let e_shoff = header.e_shoff;
        let e_flags = match e_machine {
            Machine::AArch64 => Flags::AArch64,
            Machine::X86_64 => Flags::X86_64,
            _ => return Err(ElfHeaderError::UnimplementedArchitecture),
        };
        let e_ehsize = header.e_ehsize;
        let e_phentsize = header.e_phentsize;
        let e_phnum = header.e_phnum;
        let e_shentsize = header.e_shentsize;
        let e_shnum = header.e_shnum;
        let e_shstrndx = header.e_shstrndx;

        Ok(Self::Elf64Header {
            e_ident,
            e_type,
            e_machine,
            e_version,
            e_entry,
            e_phoff,
            e_shoff,
            e_flags,
            e_ehsize,
            e_phentsize,
            e_phnum,
            e_shentsize,
            e_shnum,
            e_shstrndx,
            data,
        })
    }

    pub fn e_ident(&self) -> identity::ElfIdentity {
        match self {
            Self::Elf32Header { e_ident, .. } => *e_ident,
            Self::Elf64Header { e_ident, .. } => *e_ident,
        }
    }
    pub fn e_type(&self) -> Type {
        match self {
            Self::Elf32Header { e_type, .. } => *e_type,
            Self::Elf64Header { e_type, .. } => *e_type,
        }
    }
    pub fn e_machine(&self) -> Machine {
        match self {
            Self::Elf32Header { e_machine, .. } => *e_machine,
            Self::Elf64Header { e_machine, .. } => *e_machine,
        }
    }
    pub fn e_version(&self) -> Version {
        match self {
            Self::Elf32Header { e_version, .. } => *e_version,
            Self::Elf64Header { e_version, .. } => *e_version,
        }
    }
    pub fn e_entry(&self) -> u64 {
        match self {
            Self::Elf32Header { e_entry, .. } => *e_entry as u64,
            Self::Elf64Header { e_entry, .. } => *e_entry,
        }
    }
    pub fn e_phoff(&self) -> u64 {
        match self {
            Self::Elf32Header { e_phoff, .. } => *e_phoff as u64,
            Self::Elf64Header { e_phoff, .. } => *e_phoff,
        }
    }
    pub fn e_shoff(&self) -> u64 {
        match self {
            Self::Elf32Header { e_shoff, .. } => *e_shoff as u64,
            Self::Elf64Header { e_shoff, .. } => *e_shoff,
        }
    }
    pub fn e_flags(&self) -> Flags {
        match self {
            Self::Elf32Header { e_flags, .. } => *e_flags,
            Self::Elf64Header { e_flags, .. } => *e_flags,
        }
    }
    pub fn e_ehsize(&self) -> u16 {
        match self {
            Self::Elf32Header { e_ehsize, .. } => *e_ehsize,
            Self::Elf64Header { e_ehsize, .. } => *e_ehsize,
        }
    }
    pub fn e_phentsize(&self) -> u16 {
        match self {
            Self::Elf32Header { e_phentsize, .. } => *e_phentsize,
            Self::Elf64Header { e_phentsize, .. } => *e_phentsize,
        }
    }
    pub fn e_phnum(&self) -> u16 {
        match self {
            Self::Elf32Header { e_phnum, .. } => *e_phnum,
            Self::Elf64Header { e_phnum, .. } => *e_phnum,
        }
    }
    pub fn e_shentsize(&self) -> u16 {
        match self {
            Self::Elf32Header { e_shentsize, .. } => *e_shentsize,
            Self::Elf64Header { e_shentsize, .. } => *e_shentsize,
        }
    }
    pub fn e_shnum(&self) -> u16 {
        match self {
            Self::Elf32Header { e_shnum, .. } => *e_shnum,
            Self::Elf64Header { e_shnum, .. } => *e_shnum,
        }
    }
    pub fn e_shstrndx(&self) -> u16 {
        match self {
            Self::Elf32Header { e_shstrndx, .. } => *e_shstrndx,
            Self::Elf64Header { e_shstrndx, .. } => *e_shstrndx,
        }
    }
}
