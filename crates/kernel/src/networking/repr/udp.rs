use byteorder::{ByteOrder, NetworkEndian};

use alloc::vec;
use alloc::vec::Vec;

use super::Ipv4Address;
use crate::networking::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub src_port: u16,
    pub dst_port: u16,
    pub src_ip: Ipv4Address,
    pub dst_ip: Ipv4Address,
    pub length: u16,
    pub checksum: u16,
    pub payload: Vec<u8>,
}

impl Packet {
    pub const HEADER_LEN: usize = 8;

    pub fn new(
        src_port: u16,
        dst_port: u16,
        payload: Vec<u8>,
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
    ) -> Self {
        let length = Self::HEADER_LEN as u16 + payload.len() as u16;
        Packet {
            src_port,
            dst_port,
            src_ip,
            dst_ip,
            length,
            checksum: 0,
            payload,
        }
    }
    // Deserialize a UDP packet from a byte buffer
    pub fn deserialize(buf: &[u8]) -> Result<Self> {
        if buf.len() < 8 {
            return Err(Error::Malformed);
        }

        let src_port = NetworkEndian::read_u16(&buf[0..2]);
        let dst_port = NetworkEndian::read_u16(&buf[2..4]);
        let length = NetworkEndian::read_u16(&buf[4..6]);
        let checksum = NetworkEndian::read_u16(&buf[6..8]);

        // println!("length {} buf length {}", length, buf.len());
        if length < 8 || buf.len() != length as usize {
            return Err(Error::Malformed);
        }

        let payload = buf[8..length as usize].to_vec();

        Ok(Packet {
            src_port,
            dst_port,
            src_ip: Ipv4Address::new([0, 0, 0, 0]),
            dst_ip: Ipv4Address::new([0, 0, 0, 0]),
            length,
            checksum,
            payload,
        })
    }

    // Serialize a UDP packet to a byte vector
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; Self::HEADER_LEN + self.payload.len()];

        NetworkEndian::write_u16(&mut buf[0..2], self.src_port);
        NetworkEndian::write_u16(&mut buf[2..4], self.dst_port);
        NetworkEndian::write_u16(&mut buf[4..6], self.length);
        NetworkEndian::write_u16(&mut buf[6..8], 0); // temporary checksum = 0

        buf[8..].copy_from_slice(&self.payload);

        // Now compute checksum and overwrite it
        let checksum = Self::compute_checksum_raw(self.src_ip, self.dst_ip, &buf);

        NetworkEndian::write_u16(&mut buf[6..8], checksum);

        buf
    }

    fn compute_checksum_raw(src_ip: Ipv4Address, dst_ip: Ipv4Address, udp_segment: &[u8]) -> u16 {
        let mut pseudo_header = Vec::with_capacity(12);
        pseudo_header.extend_from_slice(&src_ip.as_bytes());
        pseudo_header.extend_from_slice(&dst_ip.as_bytes());
        pseudo_header.push(0);
        pseudo_header.push(17); // UDP protocol number
        pseudo_header.extend_from_slice(&(udp_segment.len() as u16).to_be_bytes());

        let mut data = pseudo_header;
        data.extend_from_slice(udp_segment);

        if data.len() % 2 != 0 {
            data.push(0);
        }

        let mut sum = 0u32;
        for chunk in data.chunks(2) {
            let word = u16::from_be_bytes([chunk[0], chunk[1]]);
            sum = sum.wrapping_add(word as u32);
        }

        while (sum >> 16) != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }

        let checksum = !(sum as u16);
        if checksum == 0 {
            0xFFFF
        } else {
            checksum
        }
    }
}
