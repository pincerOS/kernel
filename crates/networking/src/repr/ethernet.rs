/*
+-------------------+-------------------+-------------------+
|  Preamble (7B)   |  SFD (1B)         | Destination MAC (6B) |
+-------------------+-------------------+-------------------+
| Source MAC (6B)  | EtherType/Length (2B)                  |
+-------------------+----------------------------------------+
|                  Payload (46 - 1500B)                     |
+-----------------------------------------------------------+
|                  Frame Check Sequence (FCS - 4B)         |
+-----------------------------------------------------------+

ethernet is the lowest layer (in this stack), other physical data transfer layers are not 
implemented (e.g 802.11) 

this file mainly lays out the specifications and representation for ethernet
*/

use byteorder::{ByteOrder, NetworkEndian};
use core::fmt;

use crate::{Result, Error};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct Address([u8; 6]);

impl Address {
    pub const BROADCAST: Address = Address([0xFF; 6]);
    
    // converts bytes to address
    pub fn from_bytes(data: &[u8]) -> Result<Address> {
        if data.len() != 6 {
            return Err(Error::Malformed);
        }
        let mut bytes = [0; 6];
        bytes.copy_from_slice(data);
        Ok(Address(bytes))
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

// NOTE: add to here if you want to add more supported protocols
pub mod eth_types {
    pub const IPV4: u16 = 0x800;
    pub const ARP: u16 = 0x806;
}

mod fields {
    use core::ops::{Range, RangeFrom};

    pub const DST_ADDR: Range<usize> = 0 .. 6;
    pub const SRC_ADDR: Range<usize> = 6 .. 12;
    pub const PAYLOAD_TYPE: Range<usize> = 12 .. 14;
    pub const PAYLOAD: RangeFrom<usize> = 14 ..;
}

// NOTE: ty smoltcp for this hack, this lets us basically use any representation for the actual
// data in the packet and handle just the logic around it
#[derive(Debug)]
pub struct Frame<T: AsRef<[u8]>> {
    buffer: T,
}

// calling .as_ref() will give us a &[u8] from the buffer to work with
impl<T: AsRef<[u8]>> AsRef<[u8]> for Frame<T> {
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }
}

// calling .as_mut() will give us a &[u8] mutable, also it will only exist when T is mutable
impl<T: AsRef<[u8]> + AsMut<[u8]>> AsMut<[u8]> for Frame<T> {
    fn as_mut(&mut self) -> &mut [u8] {
        self.buffer.as_mut()
    }
}

impl<T: AsRef<[u8]>> Frame<T> {
    pub const HEADER_LEN: usize = 14;
    pub const MAX_FRAME_LEN: usize = 1518;

    // byte buffer -> ethernet frame
    pub fn try_new(buffer: T) -> Result<Frame<T>> {
        if buffer.as_ref().len() < Self::HEADER_LEN || buffer.as_ref().len() > Self::MAX_FRAME_LEN {
            Err(Error::Malformed)
        } else {
            Ok(Frame{buffer})
        }
    }

    pub fn buffer_len(payload_len: usize) -> usize {
        Self::HEADER_LEN + payload_len
    }

    pub fn dst_addr(&self) -> Address {
        let data = self.buffer.as_ref();
        Address::from_bytes(&data[fields::DST_ADDR]).unwrap()
    }

    pub fn src_addr(&self) -> Address {
        let data = self.buffer.as_ref();
        Address::from_bytes(&data[fields::SRC_ADDR]).unwrap()
    }

    pub fn payload_type(&self) -> u16 {
        let data = self.buffer.as_ref();
        NetworkEndian::read_u16(&data[fields::PAYLOAD_TYPE])
    }

    pub fn payload(&self) -> &[u8] {
        &self.buffer.as_ref()[fields::PAYLOAD]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Frame<T> {
    pub fn set_dst_addr(&mut self, addr: Address) {
        let data = self.buffer.as_mut();
        data[fields::DST_ADDR].copy_from_slice(addr.as_bytes())
    }

    pub fn set_src_addr(&mut self, addr: Address) {
        let data = self.buffer.as_mut();
        data[fields::SRC_ADDR].copy_from_slice(addr.as_bytes())
    }

    pub fn set_payload_type(&mut self, payload_type: u16) {
        let data = self.buffer.as_mut();
        NetworkEndian::write_u16(&mut data[fields::PAYLOAD_TYPE], payload_type.into())

    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.buffer.as_mut()[fields::PAYLOAD]
    }
}

