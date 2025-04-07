use byteorder::{NetworkEndian, ByteOrder};

use core::cmp::Ordering;
use core::ops::Deref;
use core::fmt::{Display, Formatter, Result as FmtResult};
use core::result::Result as StdResult;
use core::str::FromStr;

use alloc::vec::Vec;

use crate::utils::checksum::internet_checksum;
use crate::{Error, Result};

// https://en.wikipedia.org/wiki/IPv4
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Address([u8; 4]);

impl Address {
    pub fn new(addr: [u8; 4]) -> Address {
        Address(addr)
    }

    pub fn from_bytes(addr: &[u8]) -> Result<Address> {
        if addr.len() != 4 {
            return Err(Error::Exhausted);
        }

        let mut _addr: [u8; 4] = [0; 4];
        _addr.clone_from_slice(addr);
        Ok(Address(_addr))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn as_int(&self) -> u32 {
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

impl From<u32> for Address {
    fn from(addr: u32) -> Address {
        let mut bytes = [0; 4];
        NetworkEndian::write_u32(&mut bytes[..], addr);
        Address(bytes)
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
    // ipv4 + subnetmask
    // TODO: handle errors if subnetlen > 32
    pub fn new(address: Address, subnet_len: usize) -> AddressCidr {
        assert!(subnet_len <= 32);

        AddressCidr {
            address,
            subnet_len: subnet_len as u32,
        }
    }

    pub fn is_member(&self, address: Address) -> bool {
        let mask = !(0xFFFFFFFF >> self.subnet_len);
        (address.as_int() & mask) == (self.address.as_int() & mask)
    }
    pub fn is_broadcast(&self, address: Address) -> bool {
        address == self.broadcast()
    }
    pub fn broadcast(&self) -> Address {
        let mask = !(0xFFFFFFFF >> self.subnet_len);
        let addr = (self.address.as_int() & mask) | (!mask);
        Address::from(addr)
    }
}

impl Display for AddressCidr {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}/{}", self.address, self.subnet_len)
    }
}

impl Deref for AddressCidr {
    type Target = Address;

    fn deref(&self) -> &Address {
        &self.address
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Protocol {
    ICMP = Protocols::ICMP,
    UDP = Protocols::UDP,
    // TCP = protocols::TCP,
    __Nonexhaustive,
}

// An IPv4 header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repr {
    pub src_addr: Address,
    pub dst_addr: Address,
    pub protocol: Protocol,
    pub payload_len: u16,
}

impl Repr {
    pub fn buffer_len(&self) -> usize {
        Packet::<&[u8]>::MIN_HEADER_LEN + (self.payload_len as usize)
    }

    // try to deserialize a packet into an IPv4 header.
    pub fn deserialize<T>(packet: &Packet<T>) -> Result<Repr>
    where
        T: AsRef<[u8]>,
    {
        Ok(Repr {
            src_addr: packet.src_addr(),
            dst_addr: packet.dst_addr(),
            protocol: match packet.protocol() {
                Protocols::ICMP => Protocol::ICMP,
                // Protocols::TCP => Protocol::TCP,
                Protocols::UDP => Protocol::UDP,
                _ => return Err(Error::Malformed),
            },
            payload_len: packet.payload().len() as u16,
        })
    }

    // serialize + checksum update
    pub fn serialize<T>(&self, packet: &mut Packet<T>)
    where
        T: AsRef<[u8]> + AsMut<[u8]>,
    {
        packet.set_ip_version(4);
        packet.set_header_len(5);
        packet.set_dscp(0);
        packet.set_ecn(0);
        packet.set_packet_len(20 + self.payload_len as u16);
        packet.set_identification(0);
        packet.set_flags(flags::DONT_FRAGMENT);
        packet.set_fragment_offset(0);
        packet.set_ttl(64);
        packet.set_protocol(self.protocol as u8);
        packet.set_header_checksum(0);
        packet.set_src_addr(self.src_addr);
        packet.set_dst_addr(self.dst_addr);

        let checksum = packet.gen_header_checksum();
        packet.set_header_checksum(checksum);
    }

    pub fn gen_checksum_with_pseudo_header(&self, buffer: &[u8]) -> u16 {
        let mut ip_pseudo_header = [0; 12];
        (&mut ip_pseudo_header[0..4]).copy_from_slice(self.src_addr.as_bytes());
        (&mut ip_pseudo_header[4..8]).copy_from_slice(self.dst_addr.as_bytes());
        ip_pseudo_header[9] = self.protocol as u8;
        NetworkEndian::write_u16(&mut ip_pseudo_header[10..12], self.payload_len);
    
        let mut full_buffer: Vec<u8> = ip_pseudo_header.to_vec();
        full_buffer.extend_from_slice(buffer);
    
        internet_checksum(&full_buffer)
    }

}

#[allow(non_snake_case)]
pub mod Protocols {
    pub const ICMP: u8 = 1;
    // pub const TCP: u8 = 6;
    pub const UDP: u8 = 17;
}

pub mod flags {
    pub const DONT_FRAGMENT: u8 = 0b00000010;
    pub const NOT_LAST: u8 = 0b00000001;
}

// https://en.wikipedia.org/wiki/IPv4
mod fields {
    use core::ops::Range;

    pub const IP_VERSION_AND_HEADER_LEN: usize = 0;
    pub const DSCP_AND_ECN: usize = 1;
    pub const PACKET_LEN: Range<usize> = 2 .. 4;
    pub const IDENTIFICATION: Range<usize> = 4 .. 6;
    pub const FLAGS: usize = 6;
    pub const FRAG_OFFSET: Range<usize> = 6 .. 8;
    pub const TTL: usize = 8;
    pub const PROTOCOL: usize = 9;
    pub const CHECKSUM: Range<usize> = 10 .. 12;
    pub const SRC_ADDR: Range<usize> = 12 .. 16;
    pub const DST_ADDR: Range<usize> = 16 .. 20;
}

#[derive(Debug)]
pub struct Packet<T: AsRef<[u8]>> {
    buffer: T,
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for Packet<T> {
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> AsMut<[u8]> for Packet<T> {
    fn as_mut(&mut self) -> &mut [u8] {
        self.buffer.as_mut()
    }
}

impl<T: AsRef<[u8]>> Packet<T> {
    pub const MIN_HEADER_LEN: usize = 20;

    // WARN: enforce check_encoding() for untrusted srcs
    pub fn try_new(buffer: T) -> Result<Packet<T>> {
        if buffer.as_ref().len() < Self::MIN_HEADER_LEN {
            Err(Error::Exhausted)
        } else {
            Ok(Packet { buffer })
        }
    }

    pub fn buffer_len(payload_len: usize) -> usize {
        20 + payload_len
    }

    pub fn check_encoding(&self) -> Result<()> {
        if (self.packet_len() as usize) > self.buffer.as_ref().len()
            || ((self.header_len() * 4) as usize) < Self::MIN_HEADER_LEN
            || ((self.header_len() * 4) as usize) > self.buffer.as_ref().len()
            || self.ip_version() != 4
        {
            Err(Error::Malformed)
        } else if self.gen_header_checksum() != 0 {
            Err(Error::Checksum)
        } else {
            Ok(())
        }
    }

    pub fn gen_header_checksum(&self) -> u16 {
        let header_len = (self.header_len() * 4) as usize;
        internet_checksum(&self.buffer.as_ref()[.. header_len])
    }

    pub fn ip_version(&self) -> u8 {
        (self.buffer.as_ref()[fields::IP_VERSION_AND_HEADER_LEN] & 0xF0) >> 4
    }

    pub fn header_len(&self) -> u8 {
        self.buffer.as_ref()[fields::IP_VERSION_AND_HEADER_LEN] & 0x0F
    }

    pub fn dscp(&self) -> u8 {
        (self.buffer.as_ref()[fields::DSCP_AND_ECN] & 0xFC) >> 2
    }

    pub fn ecn(&self) -> u8 {
        self.buffer.as_ref()[fields::DSCP_AND_ECN] & 0x03
    }

    pub fn packet_len(&self) -> u16 {
        NetworkEndian::read_u16(&self.buffer.as_ref()[fields::PACKET_LEN])
    }

    pub fn identification(&self) -> u16 {
        NetworkEndian::read_u16(&self.buffer.as_ref()[fields::IDENTIFICATION])
    }

    pub fn flags(&self) -> u8 {
        (self.buffer.as_ref()[fields::FLAGS] & 0xE0) >> 5
    }

    pub fn fragment_offset(&self) -> u16 {
        let frag_offset_slice = &self.buffer.as_ref()[fields::FRAG_OFFSET];
        let mut frag_offset_only: [u8; 2] = [0; 2];
        frag_offset_only[0] = frag_offset_slice[0] & 0x1F; // Clear flags!
        frag_offset_only[1] = frag_offset_slice[1];
        NetworkEndian::read_u16(&frag_offset_only[..])
    }

    pub fn ttl(&self) -> u8 {
        self.buffer.as_ref()[fields::TTL]
    }

    pub fn protocol(&self) -> u8 {
        self.buffer.as_ref()[fields::PROTOCOL]
    }

    pub fn header_checksum(&self) -> u16 {
        NetworkEndian::read_u16(&self.buffer.as_ref()[fields::CHECKSUM])
    }

    pub fn src_addr(&self) -> Address {
        Address::from_bytes(&self.buffer.as_ref()[fields::SRC_ADDR]).unwrap()
    }

    pub fn dst_addr(&self) -> Address {
        Address::from_bytes(&self.buffer.as_ref()[fields::DST_ADDR]).unwrap()
    }

    pub fn payload(&self) -> &[u8] {
        let header_len = (self.header_len() * 4) as usize;
        let packet_len = self.packet_len() as usize;
        &self.buffer.as_ref()[header_len .. packet_len]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    pub fn set_ip_version(&mut self, version: u8) {
        self.buffer.as_mut()[fields::IP_VERSION_AND_HEADER_LEN] &= !0xF0;
        self.buffer.as_mut()[fields::IP_VERSION_AND_HEADER_LEN] |= version << 4;
    }

    pub fn set_header_len(&mut self, header_len: u8) {
        self.buffer.as_mut()[fields::IP_VERSION_AND_HEADER_LEN] &= !0x0F;
        self.buffer.as_mut()[fields::IP_VERSION_AND_HEADER_LEN] |= header_len & 0x0F;
    }

    pub fn set_dscp(&mut self, dscp: u8) {
        self.buffer.as_mut()[fields::DSCP_AND_ECN] &= !0xFC;
        self.buffer.as_mut()[fields::DSCP_AND_ECN] |= dscp << 2;
    }

    pub fn set_ecn(&mut self, ecn: u8) {
        self.buffer.as_mut()[fields::DSCP_AND_ECN] &= !0x03;
        self.buffer.as_mut()[fields::DSCP_AND_ECN] |= ecn & 0x03;
    }

    pub fn set_packet_len(&mut self, packet_len: u16) {
        NetworkEndian::write_u16(&mut self.buffer.as_mut()[fields::PACKET_LEN], packet_len)
    }

    pub fn set_identification(&mut self, id: u16) {
        NetworkEndian::write_u16(&mut self.buffer.as_mut()[fields::IDENTIFICATION], id)
    }

    pub fn set_flags(&mut self, flags: u8) {
        self.buffer.as_mut()[fields::FLAGS] &= 0x1F;
        self.buffer.as_mut()[fields::FLAGS] |= flags << 5
    }

    pub fn set_fragment_offset(&mut self, frag_offset: u16) {
        let flags = self.flags();
        NetworkEndian::write_u16(&mut self.buffer.as_mut()[fields::FRAG_OFFSET], frag_offset);
        self.set_flags(flags);
    }

    pub fn set_ttl(&mut self, ttl: u8) {
        self.buffer.as_mut()[fields::TTL] = ttl;
    }

    pub fn set_protocol(&mut self, protocol: u8) {
        self.buffer.as_mut()[fields::PROTOCOL] = protocol;
    }

    pub fn set_header_checksum(&mut self, header_checksum: u16) {
        NetworkEndian::write_u16(&mut self.buffer.as_mut()[fields::CHECKSUM], header_checksum)
    }

    pub fn set_src_addr(&mut self, addr: Address) {
        self.buffer.as_mut()[fields::SRC_ADDR]
        .copy_from_slice(addr.as_bytes());
    }

    pub fn set_dst_addr(&mut self, addr: Address) {
        self.buffer.as_mut()[fields::DST_ADDR]
        .copy_from_slice(addr.as_bytes());
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        let header_len = (self.header_len() * 4) as usize;
        let packet_len = self.packet_len() as usize;
        &mut self.buffer.as_mut()[header_len .. packet_len]
    }
}
