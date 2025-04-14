use byteorder::{ByteOrder, NetworkEndian};

use alloc::vec;
use alloc::vec::Vec;

use super::Ipv4Address;
use crate::networking::utils::checksum::internet_checksum;
use crate::networking::{Error, Result};

#[allow(non_snake_case)]
pub mod Flags {
    pub const TCP_FIN: u8 = 0x01;
    pub const TCP_SYN: u8 = 0x02;
    pub const TCP_RST: u8 = 0x04;
    pub const TCP_PSH: u8 = 0x08;
    pub const TCP_ACK: u8 = 0x10;
    pub const TCP_URG: u8 = 0x20;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_number: u32,
    pub ack_number: u32,
    pub data_offset: u8, // top 4 bits
    pub flags: u8,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
    pub src_ip: Ipv4Address,
    pub dst_ip: Ipv4Address,
    pub payload: Vec<u8>,
}

impl Packet {
    pub const HEADER_LEN: usize = 20;

    pub fn new(
        src_port: u16,
        dst_port: u16,
        seq_number: u32,
        ack_number: u32,
        flags: u8,
        window_size: u16,
        payload: Vec<u8>,
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
    ) -> Self {
        Packet {
            src_port,
            dst_port,
            seq_number,
            ack_number,
            data_offset: 5, // 5 * 4 = 20 bytes
            flags,
            window_size,
            checksum: 0,
            urgent_ptr: 0,
            src_ip,
            dst_ip,
            payload,
        }
    }

    pub fn deserialize(buf: &[u8]) -> Result<Self> {
        if buf.len() < Self::HEADER_LEN {
            return Err(Error::Malformed);
        }

        let src_port = NetworkEndian::read_u16(&buf[0..2]);
        let dst_port = NetworkEndian::read_u16(&buf[2..4]);
        let seq_number = NetworkEndian::read_u32(&buf[4..8]);
        let ack_number = NetworkEndian::read_u32(&buf[8..12]);
        let data_offset = buf[12] >> 4;
        let flags = buf[13];
        let window_size = NetworkEndian::read_u16(&buf[14..16]);
        let checksum = NetworkEndian::read_u16(&buf[16..18]);
        let urgent_ptr = NetworkEndian::read_u16(&buf[18..20]);

        if data_offset < 5 {
            return Err(Error::Malformed);
        }

        let header_len = (data_offset as usize) * 4;
        if buf.len() < header_len {
            return Err(Error::Malformed);
        }

        let payload = buf[header_len..].to_vec();

        Ok(Packet {
            src_port,
            dst_port,
            seq_number,
            ack_number,
            data_offset,
            flags,
            window_size,
            checksum,
            urgent_ptr,
            src_ip: Ipv4Address::new([0, 0, 0, 0]),
            dst_ip: Ipv4Address::new([0, 0, 0, 0]),
            payload,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let header_len = Self::HEADER_LEN;
        let mut buf = vec![0u8; header_len + self.payload.len()];

        NetworkEndian::write_u16(&mut buf[0..2], self.src_port);
        NetworkEndian::write_u16(&mut buf[2..4], self.dst_port);
        NetworkEndian::write_u32(&mut buf[4..8], self.seq_number);
        NetworkEndian::write_u32(&mut buf[8..12], self.ack_number);
        buf[12] = (self.data_offset << 4) & 0xF0;
        buf[13] = self.flags;
        NetworkEndian::write_u16(&mut buf[14..16], self.window_size);
        NetworkEndian::write_u16(&mut buf[16..18], 0); // temporary checksum
        NetworkEndian::write_u16(&mut buf[18..20], self.urgent_ptr);

        buf[header_len..].copy_from_slice(&self.payload);

        let checksum = Self::compute_checksum_raw(self.src_ip, self.dst_ip, &buf);
        NetworkEndian::write_u16(&mut buf[16..18], checksum);

        buf
    }

    fn compute_checksum_raw(src_ip: Ipv4Address, dst_ip: Ipv4Address, tcp_segment: &[u8]) -> u16 {
        let mut pseudo_header = Vec::with_capacity(12);
        pseudo_header.extend_from_slice(&src_ip.as_bytes());
        pseudo_header.extend_from_slice(&dst_ip.as_bytes());
        pseudo_header.push(0);
        pseudo_header.push(6); // TCP protocol number
        pseudo_header.extend_from_slice(&(tcp_segment.len() as u16).to_be_bytes());

        let mut data = pseudo_header;
        data.extend_from_slice(tcp_segment);

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
