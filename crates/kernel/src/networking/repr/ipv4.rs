use byteorder::{NetworkEndian, ByteOrder};

use core::cmp::Ordering;
use core::ops::Deref;
use core::fmt::{Display, Formatter, Result as FmtResult};
use core::result::Result as StdResult;
use core::str::FromStr;

use alloc::vec;
use alloc::vec::Vec;

use crate::networking::utils::checksum::internet_checksum;
use crate::networking::{Error, Result};

// https://en.wikipedia.org/wiki/IPv4
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Address([u8; 4]);

impl Address {
    pub fn new(addr: [u8; 4]) -> Address {
        Address(addr)
    }

    pub fn from_bytes(addr: &[u8]) -> Result<Address> {
        if addr.len() != 4 {
            return Err(Error::Malformed);
        }

        let mut _addr: [u8; 4] = [0; 4];
        _addr.clone_from_slice(addr);
        Ok(Address(_addr))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
    
    pub fn from_u32(addr: u32) -> Address {
        let mut bytes = [0; 4];
        NetworkEndian::write_u32(&mut bytes[..], addr);
        Address(bytes)
    }

    pub fn as_u32(&self) -> u32 {
        NetworkEndian::read_u32(&self.0[..])
    }

    // check classes, see cidr for more
    pub fn is_unicast(&self) -> bool {
        !(self.is_multicast() || self.is_reserved())
    }
    pub fn is_multicast(&self) -> bool {
        (self.0[0] & 0b11100000) == 0b11100000
    }
    pub fn is_reserved(&self) -> bool {
        (self.0[0] & 0b11110000) == 0b11110000
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

impl Ord for Address {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for Address {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// NOTE: str must be in format "A.B.C.D"
impl FromStr for Address {
    type Err = ();

    fn from_str(addr: &str) -> StdResult<Address, Self::Err> {
        let (bytes, unknown): (Vec<_>, Vec<_>) = addr.split(".")
            .map(|token| token.parse::<u8>())
            .partition(|byte| !byte.is_err());

        if bytes.len() != 4 || unknown.len() > 0 {
            return Err(());
        }

        let bytes: Vec<_> = bytes.into_iter().map(|byte| byte.unwrap()).collect();

        let mut ipv4: [u8; 4] = [0; 4];
        ipv4.clone_from_slice(&bytes);

        Ok(Address::new(ipv4))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AddressCidr {
    address: Address,
    subnet_len: u32,
}

impl AddressCidr {
    pub fn new(address: Address, subnet_len: u32) -> Result<AddressCidr> {
        if subnet_len <= 32 {
            return Err(Error::Malformed);
        }

        Ok(AddressCidr {
            address,
            subnet_len,
        })
    }

    pub fn is_member(&self, address: Address) -> bool {
        let mask = !(0xFFFFFFFF >> self.subnet_len);
        (address.as_u32() & mask) == (self.address.as_u32() & mask)
    }
    pub fn is_broadcast(&self, address: Address) -> bool {
        address == self.broadcast()
    }
    pub fn broadcast(&self) -> Address {
        let mask = !(0xFFFFFFFF >> self.subnet_len);
        let addr = (self.address.as_u32() & mask) | (!mask);
        Address::from_u32(addr)
    }
}

impl Display for AddressCidr {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}/{}", self.address, self.subnet_len)
    }
}

// we only want the address when we dereference this
impl Deref for AddressCidr {
    type Target = Address;

    fn deref(&self) -> &Address {
        &self.address
    }
}

#[allow(non_snake_case)]
pub mod Protocols {
    pub const ICMP: u8 = 1;
    pub const TCP: u8 = 6;
    pub const UDP: u8 = 17;
}

pub mod flags {
    pub const DONT_FRAGMENT: u8 = 0b00000010;
    pub const NOT_LAST: u8 = 0b00000001;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Protocol {
    ICMP = Protocols::ICMP,
    UDP = Protocols::UDP,
    TCP = Protocols::TCP,
    __Nonexhaustive,
}

pub struct Packet {
    pub version: u8,         // 4 bits (always 4 for IPv4)
    pub ihl: u8,             // 4 bits (in 32-bit words, min value = 5)
    pub dscp: u8,            // 6 bits (type of service), we use full 8 bits here
    pub total_len: u16,      // includes header + payload
    pub id: u16,             // identification for frag
    pub flags: u8,           // 3 bits (upper bits of this field)
    pub frag_offset: u16,    // lower 13 bits used
    pub ttl: u8,             // time to live
    pub protocol: Protocol,
    pub checksum: u16,       // computed on serialization
    pub src_addr: Address,
    pub dst_addr: Address,
    pub payload: Vec<u8>,
}

impl Packet {
    const MIN_HEADER_LEN: usize = 20; // 5 * 4 = 20 bytes, minimum

    /// Simple constructor for typical user usage
    pub fn new(src_addr: Address, dst_addr: Address, protocol: Protocol, payload: Vec<u8>) -> Self {
        Self {
            version: 4,
            ihl: Self::MIN_HEADER_LEN as u8, 
            dscp: 0,
            total_len: (Self::MIN_HEADER_LEN + payload.len()) as u16,
            id: 0,
            flags: 0,
            frag_offset: 0,
            ttl: 64,
            protocol,
            checksum: 0,
            src_addr,
            dst_addr,
            payload,
        }
    }

    pub fn deserialize(buf: &[u8]) -> Result<Self> {
        if buf.len() < Self::MIN_HEADER_LEN {
            return Err(Error::Malformed);
        }

        let version_ihl = buf[0];
        let version = version_ihl >> 4;
        let ihl = version_ihl & 0x0F;

        if version != 4 || ihl < 5 {
            return Err(Error::Unsupported);
        }

        let header_len = (ihl as usize) * 4;
        if buf.len() < header_len {
            return Err(Error::Malformed);
        }

        let dscp = buf[1];
        let total_len = NetworkEndian::read_u16(&buf[2..4]);
        let id = NetworkEndian::read_u16(&buf[4..6]);

        let flags_fragment = NetworkEndian::read_u16(&buf[6..8]);
        let flags = (flags_fragment >> 13) as u8;
        let frag_offset = flags_fragment & 0x1FFF;

        let ttl = buf[8];
        let proto = buf[9];
        let checksum = NetworkEndian::read_u16(&buf[10..12]);

        let protocol = match proto {
            Protocols::ICMP => Protocol::ICMP,
            Protocols::UDP => Protocol::UDP,
            Protocols::TCP => Protocol::TCP,
            _ => return Err(Error::Unsupported),
        };

        let src_addr = Address::from_bytes(&buf[12..16])?;
        let dst_addr = Address::from_bytes(&buf[16..20])?;

        if total_len as usize > buf.len() {
            return Err(Error::Malformed);
        }

        let payload = buf[header_len..(total_len as usize)].to_vec();

        Ok(Packet {
            version,
            ihl,
            dscp,
            total_len,
            id,
            flags,
            frag_offset,
            ttl,
            protocol,
            checksum,
            src_addr,
            dst_addr,
            payload,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let header_len = (self.ihl as usize) * 4;
        let total_len = header_len + self.payload.len();

        let mut buf = vec![0u8; total_len];

        buf[0] = (self.version << 4) | (self.ihl & 0x0F);
        buf[1] = self.dscp;
        NetworkEndian::write_u16(&mut buf[2..4], total_len as u16);
        NetworkEndian::write_u16(&mut buf[4..6], self.id);
        let flags_frag = ((self.flags as u16) << 13) | (self.frag_offset & 0x1FFF);
        NetworkEndian::write_u16(&mut buf[6..8], flags_frag);
        buf[8] = self.ttl;
        buf[9] = self.protocol as u8;
        NetworkEndian::write_u16(&mut buf[10..12], 0); // placeholder for checksum

        buf[12..16].copy_from_slice(self.src_addr.as_bytes());
        buf[16..20].copy_from_slice(self.dst_addr.as_bytes());

        let checksum = internet_checksum(&buf[..header_len]);
        NetworkEndian::write_u16(&mut buf[10..12], checksum);

        buf[header_len..].copy_from_slice(&self.payload);
        buf
    }

    pub fn is_valid_checksum(&self) -> bool {
        let header_len = (self.ihl as usize) * 4;
        let mut buf = vec![0u8; header_len];

        buf[0] = (self.version << 4) | (self.ihl & 0x0F);
        buf[1] = self.dscp;
        NetworkEndian::write_u16(&mut buf[2..4], self.total_len);
        NetworkEndian::write_u16(&mut buf[4..6], self.id);

        let flags_frag = ((self.flags as u16) << 13) | (self.frag_offset & 0x1FFF);
        NetworkEndian::write_u16(&mut buf[6..8], flags_frag);

        buf[8] = self.ttl;
        buf[9] = self.protocol as u8;

        // Write zero for checksum to calculate it
        NetworkEndian::write_u16(&mut buf[10..12], 0);

        buf[12..16].copy_from_slice(self.src_addr.as_bytes());
        buf[16..20].copy_from_slice(self.dst_addr.as_bytes());

        let computed = internet_checksum(&buf);
        computed == self.checksum
    }

}

