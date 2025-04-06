use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};

use super::Ipv4Repr;
use crate::{Result, Error};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repr {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
}

impl Repr {
    pub fn buffer_len(&self) -> usize {
        self.length as usize
    }

    pub fn deserialize<T>(packet: &Packet<T>) -> Repr
    where
        T: AsRef<[u8]>,
    {
        Repr {
            src_port: packet.src_port(),
            dst_port: packet.dst_port(),
            length: packet.length(),
        }
    }

    pub fn serialize<T>(&self, packet: &mut Packet<T>, ipv4_repr: &Ipv4Repr)
    where
        T: AsRef<[u8]> + AsMut<[u8]>,
    {
        packet.set_src_port(self.src_port);
        packet.set_dst_port(self.dst_port);
        packet.set_length(self.length);
        packet.set_checksum(0);

        let checksum = packet.gen_packet_checksum(ipv4_repr);
        packet.set_checksum(checksum);
    }
}

mod fields {
    use core::ops::Range;

    pub const SRC_PORT: Range<usize> = 0 .. 2;
    pub const DST_PORT: Range<usize> = 2 .. 4;
    pub const LENGTH: Range<usize> = 4 .. 6;
    pub const CHECKSUM: Range<usize> = 6 .. 8;
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
    pub const HEADER_LEN: usize = 8;

    pub const MAX_PACKET_LEN: usize = 65535;

    pub fn try_new(buffer: T) -> Result<Packet<T>> {
        let buffer_len = buffer.as_ref().len();

        if buffer_len < Self::buffer_len(0) || buffer_len > Self::MAX_PACKET_LEN {
            Err(Error::Exhausted)
        } else {
            Ok(Packet { buffer })
        }
    }

    pub fn buffer_len(payload_len: usize) -> usize {
        8 + payload_len
    }

    // fc, chcecksym, encoding
    pub fn check_encoding(&self, ipv4_repr: &Ipv4Repr) -> Result<()> {
        if self.checksum() != 0 && self.gen_packet_checksum(ipv4_repr) != 0 {
            Err(Error::Checksum)
        } else if self.length() as usize != self.buffer.as_ref().len() {
            Err(Error::Malformed)
        } else {
            Ok(())
        }
    }

    pub fn gen_packet_checksum(&self, ipv4_repr: &Ipv4Repr) -> u16 {
        ipv4_repr.gen_checksum_with_pseudo_header(self.buffer.as_ref())
    }

    pub fn src_port(&self) -> u16 {
        (&self.buffer.as_ref()[fields::SRC_PORT])
            .read_u16::<NetworkEndian>()
            .unwrap()
    }

    pub fn dst_port(&self) -> u16 {
        (&self.buffer.as_ref()[fields::DST_PORT])
            .read_u16::<NetworkEndian>()
            .unwrap()
    }

    pub fn length(&self) -> u16 {
        (&self.buffer.as_ref()[fields::LENGTH])
            .read_u16::<NetworkEndian>()
            .unwrap()
    }

    pub fn checksum(&self) -> u16 {
        (&self.buffer.as_ref()[fields::CHECKSUM])
            .read_u16::<NetworkEndian>()
            .unwrap()
    }

    pub fn payload(&self) -> &[u8] {
        &self.buffer.as_ref()[8 ..]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    pub fn set_src_port(&mut self, port: u16) {
        (&mut self.buffer.as_mut()[fields::SRC_PORT])
            .write_u16::<NetworkEndian>(port)
            .unwrap()
    }

    pub fn set_dst_port(&mut self, port: u16) {
        (&mut self.buffer.as_mut()[fields::DST_PORT])
            .write_u16::<NetworkEndian>(port)
            .unwrap()
    }

    pub fn set_length(&mut self, length: u16) {
        (&mut self.buffer.as_mut()[fields::LENGTH])
            .write_u16::<NetworkEndian>(length)
            .unwrap()
    }

    pub fn set_checksum(&mut self, checksum: u16) {
        (&mut self.buffer.as_mut()[fields::CHECKSUM])
            .write_u16::<NetworkEndian>(checksum)
            .unwrap()
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.buffer.as_mut()[8 ..]
    }
}

