use byteorder::{ByteOrder, NetworkEndian};
use alloc::vec;
use alloc::vec::Vec;

use crate::networking::{Error, Result};
use super::{EthernetAddress, Ipv4Address};


#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Request = 0x0001,
    Reply = 0x0002,
}

#[allow(non_snake_case)]
pub mod Hardware {
    pub const ETHERNET: u16 = 0x0001;
}

#[allow(non_snake_case)]
pub mod Protocols {
    pub const IPV4: u16 = 0x0800;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    // pub hw_type      1 for ethernet
    // pub proto_type   0x800 for ipv4
    // pub hw_len       len of hwaddr, for ether this is 6
    // pub proto_len    len of ipv4 addr, 4
    pub op: Operation,
    pub source_hw_addr: EthernetAddress,
    pub source_proto_addr: Ipv4Address,
    pub target_hw_addr: EthernetAddress,
    pub target_proto_addr: Ipv4Address,
}

impl Packet {
    const PACKET_LEN: usize = 28;

    pub fn deserialize(buffer: &[u8]) -> Result<Self> {
        if buffer.len() < Self::PACKET_LEN {
            return Err(Error::Malformed);
        }

        let hw_type = NetworkEndian::read_u16(&buffer[0..2]);
        let proto_type = NetworkEndian::read_u16(&buffer[2..4]);
        let hw_len = buffer[4];
        let proto_len = buffer[5];

        if hw_type != Hardware::ETHERNET || proto_type != Protocols::IPV4 {
            return Err(Error::Unsupported);
        }

        if hw_len != 6 || proto_len != 4 {
            return Err(Error::Unsupported);
        }

        let op = NetworkEndian::read_u16(&buffer[6..8]);
        let source_hw_addr = EthernetAddress::from_bytes(&buffer[8..14])?;
        let source_proto_addr = Ipv4Address::from_bytes(&buffer[14..18])?;
        let target_hw_addr = EthernetAddress::from_bytes(&buffer[18..24])?;
        let target_proto_addr = Ipv4Address::from_bytes(&buffer[24..28])?;

        let op = match op {
            0x0001 => Operation::Request,
            0x0002 => Operation::Reply,
            _ => return Err(Error::Unsupported),
        };

        Ok(Packet {
            op,
            source_hw_addr,
            source_proto_addr,
            target_hw_addr,
            target_proto_addr,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = vec![0u8; Self::PACKET_LEN];

        // currently this is what we support lol
        NetworkEndian::write_u16(&mut buffer[0..2], Hardware::ETHERNET);
        NetworkEndian::write_u16(&mut buffer[2..4], Protocols::IPV4);
        buffer[4] = 6;  // Ethernet address length
        buffer[5] = 4;  // IPv4 address length

        let op = match self.op {
            Operation::Request => 0x0001,
            Operation::Reply => 0x0002,
        };
        NetworkEndian::write_u16(&mut buffer[6..8], op);

        buffer[8..14].copy_from_slice(&self.source_hw_addr.as_bytes());
        buffer[14..18].copy_from_slice(&self.source_proto_addr.as_bytes());
        buffer[18..24].copy_from_slice(&self.target_hw_addr.as_bytes());
        buffer[24..28].copy_from_slice(&self.target_proto_addr.as_bytes());

        buffer
    }
}

