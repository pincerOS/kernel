use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};

use crate::utils::checksum::internet_checksum;
use crate::{Result, Error};

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
pub struct Repr {
    pub message: Message,
    pub payload_len: usize,
}

impl Repr {
    pub fn buffer_len(&self) -> usize {
        8 + self.payload_len
    }

    pub fn deserialize<T>(packet: &Packet<T>) -> Result<Repr>
    where
        T: AsRef<[u8]>,
    {
        let (id, seq) = (
            (&packet.header()[0 .. 2])
                .read_u16::<NetworkEndian>()
                .unwrap(),
            (&packet.header()[2 .. 4])
                .read_u16::<NetworkEndian>()
                .unwrap(),
        );

        let payload_len = packet.payload().len();

        match (packet._type(), packet.code()) {
            (0, 0) => Ok(Repr {
                message: Message::EchoReply { id, seq },
                payload_len,
            }),
            (8, 0) => Ok(Repr {
                message: Message::EchoRequest { id, seq },
                payload_len,
            }),
            (3, 3) => Ok(Repr {
                message: Message::DestinationUnreachable(DestinationUnreachable::PortUnreachable),
                payload_len,
            }),
            (11, 0) => Ok(Repr {
                message: Message::TimeExceeded(TimeExceeded::TTLExpired),
                payload_len,
            }),
            _ => Err(Error::Malformed),
        }
    }

    // Serializes the ICMP header into a packet.
    pub fn serialize<T>(&self, packet: &mut Packet<T>) -> Result<()>
    where
        T: AsRef<[u8]> + AsMut<[u8]>,
    {
        fn echo<T>(packet: &mut Packet<T>, type_of: u8, id: u16, seq: u16)
        where
            T: AsRef<[u8]> + AsMut<[u8]>,
        {
            packet.set_type(type_of);
            packet.set_code(0);

            (&mut packet.header_mut()[0 .. 2])
                .write_u16::<NetworkEndian>(id)
                .unwrap();
            (&mut packet.header_mut()[2 .. 4])
                .write_u16::<NetworkEndian>(seq)
                .unwrap();
        }

        fn error<T>(packet: &mut Packet<T>, type_of: u8, code: u8)
        where
            T: AsRef<[u8]> + AsMut<[u8]>,
        {
            packet.set_type(type_of);
            packet.set_code(code);
            let zeros = [0; 4];
            packet.header_mut().copy_from_slice(&zeros[..]);
        }

        match self.message {
            Message::EchoReply { id, seq } => echo(packet, 0, id, seq),
            Message::EchoRequest { id, seq } => echo(packet, 8, id, seq),
            Message::DestinationUnreachable(message) => {
                let code = match message {
                    DestinationUnreachable::PortUnreachable => 3,
                    _ => unreachable!(),
                };
                error(packet, 3, code);
            }
            Message::TimeExceeded(message) => {
                let code = match message {
                    TimeExceeded::TTLExpired => 0,
                    _ => unreachable!(),
                };
                error(packet, 11, code);
            }
            _ => unreachable!(),
        };

        Ok(())
    }
}

// https://en.wikipedia.org/wiki/Internet_Control_Message_Protocol
mod fields {
    use core::ops::{
        Range,
        RangeFrom,
    };

    pub const TYPE: usize = 0;

    pub const CODE: usize = 1;

    pub const CHECKSUM: Range<usize> = 2 .. 4;

    pub const HEADER: Range<usize> = 4 .. 8;

    pub const PAYLOAD: RangeFrom<usize> = 8 ..;
}

#[derive(Debug)]
pub struct Packet<T: AsRef<[u8]>> {
    buffer: T,
}

impl<T: AsRef<[u8]>> Packet<T> {
    pub const HEADER_LEN: usize = 8;
    pub const MAX_PACKET_LEN: usize = 65535;

    // WARN: need to enforce check_encoding() before operating on the packet if source is untrusted
    pub fn from_bytes(buffer: T) -> Result<Packet<T>> {
        if buffer.as_ref().len() < Self::HEADER_LEN || buffer.as_ref().len() > Self::MAX_PACKET_LEN
        {
            Err(Error::Exhausted)
        } else {
            Ok(Packet{ buffer })
        }
    }

    pub fn buffer_len(payload_len: usize) -> usize {
        Self::HEADER_LEN + payload_len
    }

    pub fn check_encoding(&self) -> Result<()> {
        if internet_checksum(self.buffer.as_ref()) != 0 {
            Err(Error::Checksum)
        } else {
            Ok(())
        }
    }

    pub fn _type(&self) -> u8 {
        self.buffer.as_ref()[fields::TYPE]
    }

    pub fn code(&self) -> u8 {
        self.buffer.as_ref()[fields::CODE]
    }

    pub fn checksum(&self) -> u16 {
        (&self.buffer.as_ref()[fields::CHECKSUM])
            .read_u16::<NetworkEndian>()
            .unwrap()
    }

    pub fn header(&self) -> &[u8] {
        &self.buffer.as_ref()[fields::HEADER]
    }

    pub fn payload(&self) -> &[u8] {
        &self.buffer.as_ref()[fields::PAYLOAD]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    pub fn set_type(&mut self, type_of: u8) {
        self.buffer.as_mut()[fields::TYPE] = type_of
    }

    pub fn set_code(&mut self, code: u8) {
        self.buffer.as_mut()[fields::CODE] = code;
    }

    pub fn set_checksum(&mut self, checksum: u16) {
        (&mut self.buffer.as_mut()[fields::CHECKSUM])
            .write_u16::<NetworkEndian>(checksum)
            .unwrap()
    }

    pub fn header_mut(&mut self) -> &mut [u8] {
        return &mut self.buffer.as_mut()[fields::HEADER];
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        return &mut self.buffer.as_mut()[fields::PAYLOAD];
    }

    pub fn gen_and_set_checksum(&mut self) {
        self.set_checksum(0);
        let checksum = internet_checksum(self.buffer.as_ref());
        self.set_checksum(checksum);
    }

}
