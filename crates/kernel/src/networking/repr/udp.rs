use byteorder::{NetworkEndian, ByteOrder};

use alloc::vec::Vec;

use super::Ipv4Repr;
use crate::networking::{Result, Error};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
    pub payload: Vec<u8>,
}

impl Packet {
    // Deserialize a UDP packet from a byte buffer
    pub fn deserialize(buf: &[u8]) -> Result<Self> {
        if buf.len() < 8 {
            return Err(Error::Malformed);
        }

        let src_port = NetworkEndian::read_u16(&buf[0..2]);
        let dst_port = NetworkEndian::read_u16(&buf[2..4]);
        let length = NetworkEndian::read_u16(&buf[4..6]);
        let checksum = NetworkEndian::read_u16(&buf[6..8]);

        // Ensure the length is at least 8 (minimum UDP header size)
        if length < 8 || buf.len() != length as usize {
            return Err(Error::Malformed);
        }

        let payload = buf[8..].to_vec();

        Ok(Packet {
            src_port,
            dst_port,
            length,
            checksum,
            payload,
        })
    }

    // Serialize a UDP packet to a byte vector
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + self.payload.len());

        // Write the UDP header
        NetworkEndian::write_u16(&mut buf, self.src_port);
        NetworkEndian::write_u16(&mut buf, self.dst_port);
        NetworkEndian::write_u16(&mut buf, self.length);
        NetworkEndian::write_u16(&mut buf, self.checksum);

        // Write the payload
        buf.extend_from_slice(&self.payload);

        buf
    }
}
