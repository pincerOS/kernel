use byteorder::{ByteOrder, NetworkEndian};

use crate::{Error, Result};
use super::{EthernetAddress, Ipv4Address};

/*
+-----------------------------------------------+
| Hardware Type (HTYPE)                        |
+-----------------------------------------------+
| Protocol Type (PTYPE)                        |
+-----------------------------------------------+
| Hardware Address Length (HLEN)               |
+-----------------------------------------------+
| Protocol Address Length (PLEN)               |
+-----------------------------------------------+
| Operation (OP)                               |
+-----------------------------------------------+
| Sender Hardware Address (SHA)                |
+-----------------------------------------------+
| Sender Protocol Address (SPA)                |
+-----------------------------------------------+
| Target Hardware Address (THA)                |
+-----------------------------------------------+
| Target Protocol Address (TPA)                |
+-----------------------------------------------+
*/

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Request = 0x0001,
    Reply = 0x0002,
}

// NOTE: for more suppored hardware and protocols, add here
#[allow(non_snake_case)]
pub mod Hardware {
    pub const ETHERNET: u16 = 0x0001;
}

#[allow(non_snake_case)]
pub mod Protocols {
    pub const IPV4: u16 = 0x0800;
}

// packet structure [https://en.wikipedia.org/wiki/Address_Resolution_Protocol]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    // pub hw_type: u16,                       // 0 
    // pub proto_type: u16,                    // 2
    // pub hw_len: u8,                         // 4
    // pub proto_len: u8,                      // 5
    pub op: Operation,                      // 6
    pub source_hw_addr: EthernetAddress,    // 8
    pub source_proto_addr: Ipv4Address,     // 14
    pub target_hw_addr: EthernetAddress,    // 18
    pub target_proto_addr: Ipv4Address,     // 24
}

impl Packet {
    pub fn buffer_len(&self) -> usize {
        28  // 8B header + 24B body
    }

    pub fn deserialize(buffer: &[u8]) -> Result<Packet> {
        if buffer.len() < 28 {
            return Err(Error::Malformed);
        }

        // read header
        // TODO: check proto and hw len, currently not checked
        let hw_type = NetworkEndian::read_u16(&buffer[0 .. 2]);
        let proto_type = NetworkEndian::read_u16(&buffer[2 .. 4]);
        // let hw_len = (&buffer[4 .. 5]).read_u8::<NetworkEndian>().unwrap();
        // let proto_len = (&buffer[5 .. 6]).read_u8::<NetworkEndian>().unwrap();
        let op = NetworkEndian::read_u16(&buffer[6 .. 8]);

        if hw_type != Hardware::ETHERNET || proto_type != Protocols::IPV4 {
            return Err(Error::Unsupported);
        }

        Ok(Packet {
            // hw_type,
            // proto_type,
            // hw_len,
            // proto_len,
            op: match op {
                0x0001 => Operation::Request,
                0x0002 => Operation::Reply,
                _ => return Err(Error::Unsupported)
            },
            source_hw_addr: EthernetAddress::from_bytes(&buffer[8 .. 14]).unwrap(),
            source_proto_addr: Ipv4Address::from_bytes(&buffer[14 .. 18]).unwrap(),
            target_hw_addr: EthernetAddress::from_bytes(&buffer[18 .. 24]).unwrap(),
            target_proto_addr: Ipv4Address::from_bytes(&buffer[24 .. 28]).unwrap()
        })
    }

    pub fn serialize(&self, buffer: &mut [u8]) -> Result<()> {
        if self.buffer_len() > buffer.len() {
            return Err(Error::Malformed);
        }

        let mut offset = 0;

        // WARN: currently, nothing else is supported, this is hardcoded and does not actually read
        // from the provided buffer
        NetworkEndian::write_u16(&mut buffer[offset..], Hardware::ETHERNET);
        offset += 2;
        NetworkEndian::write_u16(&mut buffer[offset..], Protocols::IPV4);
        offset += 2; 

        // hardware address length
        buffer[offset] = 6;
        offset += 1;
        // protocol address length
        buffer[offset] = 4;
        offset += 1; 

        // OP
        NetworkEndian::write_u16(&mut buffer[offset..], self.op as u16);
        offset += 2; 

        // addr
        let src_hw = self.source_hw_addr.as_bytes();
        let src_proto = self.source_proto_addr.as_bytes();

        buffer[offset..offset + src_hw.len()].copy_from_slice(src_hw);
        offset += src_hw.len();
        buffer[offset..offset + src_proto.len()].copy_from_slice(src_proto);
        offset += src_proto.len(); 

        let tgt_hw = self.target_hw_addr.as_bytes();
        let tgt_proto = self.target_proto_addr.as_bytes();

        buffer[offset..offset + tgt_hw.len()].copy_from_slice(tgt_hw);
        offset += tgt_hw.len(); 
        buffer[offset..offset + tgt_proto.len()].copy_from_slice(tgt_proto);

        // NOTE: need to ask alex about if there's a better practice for this
        Ok(())
    }
}
