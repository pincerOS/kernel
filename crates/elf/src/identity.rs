// https://refspecs.linuxfoundation.org/elf/gabi4+/ch4.eheader.html#elfid

use core::fmt::{self, Display};

// File identification
const EI_MAG0: usize = 0;
const EI_MAG1: usize = 1;
const EI_MAG2: usize = 2;
const EI_MAG3: usize = 3;
// File class
const EI_CLASS: usize = 4;
// Data encoding
const EI_DATA: usize = 5;
// File version
const EI_VERSION: usize = 6;
// OS/ABI identification
const EI_OSABI: usize = 7;
// ABI version
const EI_ABIVERSION: usize = 8;
// Size of e_ident[]
pub const EI_NIDENT: usize = 16;

#[derive(Debug, Copy, Clone)]
pub struct ElfIdentity<'a> {
    pub class: Class,
    pub data: DataEncoding,
    pub version: Version,
    pub os_abi: OsAbi,
    pub abi_version: u8,
    bytes: &'a [u8],
}

#[derive(Debug, Copy, Clone)]
pub enum Class {
    ELF32,
    ELF64,
}

#[derive(Debug, Copy, Clone)]
pub enum DataEncoding {
    LSB,
    MSB,
}

impl Display for DataEncoding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::LSB => write!(f, "2's complement, little endian"),
            Self::MSB => write!(f, "2's complement. big endian"),
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
            Self::Current => write!(f, "1 (current)"),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum OsAbi {
    None,
    HPUX,
    NetBSD,
    Linux,
    Solaris,
    AIX,
    IRIX,
    FreeBSD,
    TRU64,
    Modesto,
    OpenBSD,
    OpenVMS,
    NSK,
    Other(u8),
}

impl Display for OsAbi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::None => write!(f, "UNIX - System V"),
            Self::HPUX => write!(f, "HP-UX"),
            Self::NetBSD => write!(f, "NetBSD"),
            Self::Linux => write!(f, "Linux"),
            Self::Solaris => write!(f, "Solaris"),
            Self::AIX => write!(f, "AIX"),
            Self::IRIX => write!(f, "IRIX"),
            Self::FreeBSD => write!(f, "FreeBSD"),
            Self::TRU64 => write!(f, "TRU64 UNIX"),
            Self::Modesto => write!(f, "Novell Modesto"),
            Self::OpenBSD => write!(f, "OpenBSD"),
            Self::OpenVMS => write!(f, "OpenVMS"),
            Self::NSK => write!(f, "HP Non-Stop Kernel"),
            Self::Other(abi) => write!(f, "Other ({})", abi),
        }
    }
}

#[derive(Debug)]
pub enum ElfIdentityError {
    InvalidLength,
    InvalidMagic,
    InvalidClass,
    UnknownClass,
    InvalidEncoding,
    UnknownDataEncoding,
    BigEndian,
    InvalidVersion,
    UnknownVersion,
}

impl Display for ElfIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidLength => write!(f, "Invalid length"),
            Self::InvalidMagic => write!(f, "Invalid magic"),
            Self::InvalidClass => write!(f, "Invalid class"),
            Self::UnknownClass => write!(f, "Unknown class"),
            Self::InvalidEncoding => write!(f, "Invalid data encoding"),
            Self::UnknownDataEncoding => write!(f, "Unknown data encoding"),
            Self::BigEndian => write!(f, "Big-endian encoding not supported"),
            Self::InvalidVersion => write!(f, "Invalid version"),
            Self::UnknownVersion => write!(f, "Unknown version"),
        }
    }
}

impl<'a> ElfIdentity<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Result<Self, ElfIdentityError> {
        if data.len() != EI_NIDENT {
            return Err(ElfIdentityError::InvalidLength);
        }

        let magic = [data[EI_MAG0], data[EI_MAG1], data[EI_MAG2], data[EI_MAG3]];
        if magic != [0x7F, b'E', b'L', b'F'] {
            return Err(ElfIdentityError::InvalidMagic);
        }
        let class = match data[EI_CLASS] {
            0 => return Err(ElfIdentityError::InvalidClass),
            1 => Class::ELF32,
            2 => Class::ELF64,
            _ => return Err(ElfIdentityError::UnknownClass),
        };
        let data_encoding = match data[EI_DATA] {
            0 => return Err(ElfIdentityError::InvalidEncoding),
            1 => DataEncoding::LSB,
            // 2 => DataEncoding::MSB,
            // TODO: BE support? (need to interpret the rest of the file as big-endian)
            2 => return Err(ElfIdentityError::BigEndian),
            _ => return Err(ElfIdentityError::UnknownDataEncoding),
        };
        let version = match data[EI_VERSION] {
            0 => return Err(ElfIdentityError::InvalidVersion),
            1 => Version::Current,
            _ => return Err(ElfIdentityError::UnknownVersion),
        };
        let os_abi = match data[EI_OSABI] {
            0 => OsAbi::None,
            1 => OsAbi::HPUX,
            2 => OsAbi::NetBSD,
            3 => OsAbi::Linux,
            // 4 and 5?
            6 => OsAbi::Solaris,
            7 => OsAbi::AIX,
            8 => OsAbi::IRIX,
            9 => OsAbi::FreeBSD,
            10 => OsAbi::TRU64,
            11 => OsAbi::Modesto,
            12 => OsAbi::OpenBSD,
            13 => OsAbi::OpenVMS,
            14 => OsAbi::NSK,
            other => OsAbi::Other(other),
        };
        let abi_version = data[EI_ABIVERSION];

        Ok(Self {
            class,
            data: data_encoding,
            version,
            os_abi,
            abi_version,
            bytes: data,
        })
    }

    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}
