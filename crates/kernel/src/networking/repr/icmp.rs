use byteorder::{NetworkEndian, ByteOrder};

use alloc::vec;
use alloc::vec::Vec;

use crate::networking::utils::checksum::internet_checksum;
use crate::networking::{Result, Error};

/*
+-----------------------------------+
| Type (1 byte)    | Code (1 byte)   |
+-----------------------------------+
| Checksum (2 bytes)                |
+-----------------------------------+
| Identifier (2 bytes)              |
+-----------------------------------+
| Sequence Number (2 bytes)         |
+-----------------------------------+
| (Optional: Data)                  |
+-----------------------------------+
*/

// NOTE: structure by smoltcp allows us to add more different types in the future
// rust packet status struct design borrowed usrnet and smoltcp for this setup
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationUnreachable {
    PortUnreachable,
    ___Exhaustive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeExceeded {
    TTLExpired,
    ___Exhaustive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    EchoReply {
        id: u16,
        seq: u16,
    },
    EchoRequest {
        id: u16,
        seq: u16,
    },
    DestinationUnreachable(DestinationUnreachable),
    TimeExceeded(TimeExceeded),
    ___Exhaustive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    pub icmp_type: u8,
    pub code: u8,
    pub checksum: u16,
    pub message: Message,
}

impl Packet {
    pub fn new(message: Message) -> Self {
        let (icmp_type, code) = match message {
            Message::EchoReply { .. } => (0, 0),
            Message::EchoRequest { .. } => (8, 0),
            Message::DestinationUnreachable(DestinationUnreachable::PortUnreachable) => (3, 3),
            Message::DestinationUnreachable(_) => (3, 0),
            Message::TimeExceeded(TimeExceeded::TTLExpired) => (11, 0),
            Message::TimeExceeded(_) => (11, 0),
            Message::___Exhaustive => (255, 0),
        };

        // Create a temporary packet to compute the checksum
        let mut pkt = Packet {
            icmp_type,
            code,
            checksum: 0,
            message,
        };

        let buf = pkt.serialize();
        pkt.checksum = NetworkEndian::read_u16(&buf[2..4]);
        pkt
    }

    pub fn deserialize(buf: &[u8]) -> Result<Self> {
        if buf.len() < 8 {
            return Err(Error::Malformed);
        }

        let icmp_type = buf[0];
        let code = buf[1];
        let checksum = NetworkEndian::read_u16(&buf[2..4]);

        // Extract identifier and sequence number for EchoRequest/EchoReply
        let id = NetworkEndian::read_u16(&buf[4..6]);
        let seq = NetworkEndian::read_u16(&buf[6..8]);

        let message = match icmp_type {
            0 => Message::EchoReply { id, seq },     // Echo Reply
            8 => Message::EchoRequest { id, seq },   // Echo Request
            3 => {
                let unreachable_type = match code {
                    3 => DestinationUnreachable::PortUnreachable,
                    _ => DestinationUnreachable::___Exhaustive,
                };
                Message::DestinationUnreachable(unreachable_type)
            }
            11 => {
                let time_exceeded_type = match code {
                    0 => TimeExceeded::TTLExpired,
                    _ => TimeExceeded::___Exhaustive,
                };
                Message::TimeExceeded(time_exceeded_type)
            }
            _ => Message::___Exhaustive,
        };

        Ok(Packet { icmp_type, code, checksum, message })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8);

        buf.push(self.icmp_type);
        buf.push(self.code);

        buf.push(0);
        buf.push(0);

        match self.message {
            Message::EchoReply { id, seq } | Message::EchoRequest { id, seq } => {
                buf.push((id >> 8) as u8);  // High byte of identifier
                buf.push(id as u8);         // Low byte of identifier
                buf.push((seq >> 8) as u8); // High byte of sequence
                buf.push(seq as u8);        // Low byte of sequence
            }
            Message::DestinationUnreachable(_) | Message::TimeExceeded(_) => {
                buf.push(0);
                buf.push(0);
                buf.push(0);
                buf.push(0);
            }
            _ => {}
        }

        let checksum = internet_checksum(&buf);
        buf[2] = (checksum >> 8) as u8; // High byte of checksum
        buf[3] = checksum as u8;        // Low byte of checksum

        buf
    }
}
