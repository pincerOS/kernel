/*
+-------------------+-------------------+-------------------+
| Source MAC (6B)  | EtherType/Length (2B)                  |
+-------------------+----------------------------------------+
|                  Payload (46 - 1500B)                     |
+-----------------------------------------------------------+
|                  Frame Check Sequence (FCS - 4B)         |
+-----------------------------------------------------------+
*/

use byteorder::{ByteOrder, NetworkEndian};
use core::fmt;
use alloc::vec;
use alloc::vec::Vec;

use crate::networking::{Result, Error};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct Address([u8; 6]);

impl Address {
    pub const BROADCAST: Address = Address([0xFF; 6]);
    
    pub fn from_bytes(data: &[u8]) -> Result<Address> {
        if data.len() != 6 {
            return Err(Error::Malformed);
        }
        let mut bytes = [0; 6];
        bytes.copy_from_slice(data);
        Ok(Address(bytes))
    }

    pub fn from_u32(val: u32) -> Address {
        let mut bytes = [0u8; 6];
        bytes[2..].copy_from_slice(&val.to_be_bytes());
        Address(bytes)
    }

    // covert address to bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    // type checking methods
    pub fn is_unicast(&self) -> bool {
        !self.is_multicast() && !self.is_broadcast()
    }
    pub fn is_multicast(&self) -> bool {
        (self.0[0] & 0b00000001) != 0
    }
    pub fn is_broadcast(&self) -> bool {
        self.0 == Self::BROADCAST.0
    }
    pub fn is_local(&self) -> bool {
        (self.0[0] & 0b00000010) != 0
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5])
    }
}

#[allow(non_snake_case)]
pub mod EtherType {
    pub const IPV4: u16 = 0x800;
    pub const ARP: u16 = 0x806;
}

pub struct EthernetFrame {
    pub dst: Address,
    pub src: Address,
    pub ethertype: u16,
    pub payload: Vec<u8>,
    // pub fcs: Option<u32>,
}

impl EthernetFrame {
    const HEADER_LEN: usize = 14;
    const MIN_BUF_LEN: usize = 46;

    pub fn deserialize(buf: &[u8]) -> Result<Self> {
        // check if this is a possible ethernet frame
        if buf.len() < Self::HEADER_LEN + Self::MIN_BUF_LEN { 
            return Err(Error::Malformed);
        }

        let dst = Address::from_bytes(&buf[0..6])?;
        let src = Address::from_bytes(&buf[6..12])?;
        
        let ethertype = NetworkEndian::read_u16(&buf[12..14]);
        
        let payload_end = buf.len() - 4;
        let payload = buf[14..payload_end].to_vec();
        
        // let raw_fcs = u32::from_be_bytes([
        //     buf[payload_end], 
        //     buf[payload_end + 1], 
        //     buf[payload_end + 2], 
        //     buf[payload_end + 3]
        // ]);
        //     
        // let fcs = Some(raw_fcs);
        
        Ok(EthernetFrame {
            dst,
            src,
            ethertype,
            payload,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let payload_len = self.payload.len().max(Self::MIN_BUF_LEN);
        let total_len = Self::HEADER_LEN + payload_len;

        let mut buf = vec![0u8; total_len];

        buf[0..6].copy_from_slice(&self.dst.as_bytes());
        buf[6..12].copy_from_slice(&self.src.as_bytes());

        NetworkEndian::write_u16(&mut buf[12..14], self.ethertype);

        buf[14..14 + self.payload.len()].copy_from_slice(&self.payload);

        buf
    }

    pub fn size(&self) -> usize {
        let payload_len = self.payload.len().max(Self::MIN_BUF_LEN);
        Self::HEADER_LEN + payload_len
    }

    // pub fn calculate_and_fill_fcs(&mut self) { }
    // 
    // pub fn verify_fcs(&self) -> bool { }
}

fn calculate_crc32(data: &[u8]) -> u32 {
    const CRC32_POLYNOMIAL: u32 = 0x04C11DB7;
    
    let mut crc: u32 = 0xFFFFFFFF;
    
    for &byte in data {
        crc ^= (byte as u32) << 24;
        
        for _ in 0..8 {
            if (crc & 0x80000000) != 0 {
                crc = (crc << 1) ^ CRC32_POLYNOMIAL;
            } else {
                crc <<= 1;
            }
        }
    }
    
    crc ^= 0xFFFFFFFF;
    
    crc
}

