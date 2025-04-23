use super::{EthernetAddress, Ipv4Address};
use crate::networking::{Error, Result};
use alloc::vec;
use alloc::vec::Vec;
use byteorder::{ByteOrder, NetworkEndian};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageType {
    Discover = 1,
    Offer = 2,
    Request = 3,
    Decline = 4,
    Ack = 5,
    Nak = 6,
    Release = 7,
    Inform = 8,
}

// #[allow(non_snake_case)]
// pub mod Hardware {
//     pub const ETHERNET: u8 = 1;
// }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub op: u8,                   // 1 = BOOTREQUEST, 2 = BOOTREPLY
    pub htype: u8,                // Hardware address type (1 = Ethernet)
    pub hlen: u8,                 // Hardware address length (6 for Ethernet)
    pub hops: u8,                 // Used by relay agents
    pub xid: u32,                 // Transaction ID
    pub secs: u16,                // Seconds elapsed since client began acquisition
    pub flags: u16,               // Flags
    pub ciaddr: Ipv4Address,      // Client IP address
    pub yiaddr: Ipv4Address,      // Your (client) IP address
    pub siaddr: Ipv4Address,      // Next server IP address
    pub giaddr: Ipv4Address,      // Relay agent IP address
    pub chaddr: EthernetAddress,  // Client hardware address
    pub options: Vec<DhcpOption>, // DHCP options
}

pub enum DhcpParam {
    SubnetMask = 1,
    TimeOffset = 2,
    Router = 3,
    DNS = 6,
    Hostname = 12,
    DomainName = 15,
    BroadcastAddr = 28,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DhcpOption {
    pub code: u8,
    pub data: Vec<u8>,
}

impl DhcpOption {
    pub fn new(code: u8, data: Vec<u8>) -> Self {
        DhcpOption { code, data }
    }

    pub fn message_type(msg_type: MessageType) -> Self {
        DhcpOption::new(53, vec![msg_type as u8])
    }

    pub fn server_identifier(server_id: Ipv4Address) -> Self {
        DhcpOption::new(54, server_id.as_bytes().to_vec())
    }

    pub fn requested_ip(request: Ipv4Address) -> Self {
        DhcpOption::new(50, request.as_bytes().to_vec())
    }

    pub fn parameters(params: Vec<DhcpParam>) -> Self {
        DhcpOption::new(55, params.into_iter().map(|p| p as u8).collect())
    }

    pub fn end() -> Self {
        DhcpOption::new(255, vec![])
    }
}

impl Packet {
    // DHCP packet has a fixed-size header of 236 bytes before options
    const MIN_PACKET_LEN: usize = 236;
    const MAGIC_COOKIE: [u8; 4] = [99, 130, 83, 99]; // RFC 1497 magic cookie

    pub fn deserialize(buffer: &[u8]) -> Result<Self> {
        if buffer.len() < Self::MIN_PACKET_LEN {
            return Err(Error::Malformed);
        }

        let op = buffer[0];
        let htype = buffer[1];
        let hlen = buffer[2];
        let hops = buffer[3];
        let xid = NetworkEndian::read_u32(&buffer[4..8]);
        let secs = NetworkEndian::read_u16(&buffer[8..10]);
        let flags = NetworkEndian::read_u16(&buffer[10..12]);
        let ciaddr = Ipv4Address::from_bytes(&buffer[12..16])?;
        let yiaddr = Ipv4Address::from_bytes(&buffer[16..20])?;
        let siaddr = Ipv4Address::from_bytes(&buffer[20..24])?;
        let giaddr = Ipv4Address::from_bytes(&buffer[24..28])?;

        let mut chaddr_bytes = [0u8; 6];
        chaddr_bytes.copy_from_slice(&buffer[28..34]);
        let chaddr = EthernetAddress::from_bytes(&[
            chaddr_bytes[0],
            chaddr_bytes[1],
            chaddr_bytes[2],
            chaddr_bytes[3],
            chaddr_bytes[4],
            chaddr_bytes[5],
        ]);

        if buffer.len() < 240 || buffer[236..240] != Self::MAGIC_COOKIE {
            return Err(Error::Malformed);
        }

        let mut options = Vec::new();
        let mut i = 240;

        while i < buffer.len() {
            let code = buffer[i];
            i += 1;

            if code == 0 {
                // Padding
                continue;
            }

            if code == 255 {
                // End of options
                options.push(DhcpOption::end());
                break;
            }

            if i >= buffer.len() {
                return Err(Error::Malformed);
            }

            let len = buffer[i] as usize;
            i += 1;

            if i + len > buffer.len() {
                return Err(Error::Malformed);
            }

            let mut data = vec![0u8; len];
            data.copy_from_slice(&buffer[i..i + len]);
            options.push(DhcpOption::new(code, data));

            i += len;
        }

        Ok(Packet {
            op,
            htype,
            hlen,
            hops,
            xid,
            secs,
            flags,
            ciaddr,
            yiaddr,
            siaddr,
            giaddr,
            chaddr: chaddr.unwrap(),
            options,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut options_size = 4;
        for option in &self.options {
            if option.code == 255 {
                options_size += 1;
            } else {
                options_size += 2 + option.data.len(); // TLV
            }
        }

        // 236 + 4 + 3 + 9 + 1
        println!("options_size {}", options_size);

        let padding = 4 - ((Self::MIN_PACKET_LEN + options_size) % 4);

        let total_size = Self::MIN_PACKET_LEN + options_size + padding;
        let mut buffer = vec![0u8; total_size];

        // Fixed header
        buffer[0] = self.op;
        buffer[1] = self.htype;
        buffer[2] = self.hlen;
        buffer[3] = self.hops;
        NetworkEndian::write_u32(&mut buffer[4..8], self.xid);
        NetworkEndian::write_u16(&mut buffer[8..10], self.secs);
        NetworkEndian::write_u16(&mut buffer[10..12], self.flags);

        buffer[12..16].copy_from_slice(&self.ciaddr.as_bytes());
        buffer[16..20].copy_from_slice(&self.yiaddr.as_bytes());
        buffer[20..24].copy_from_slice(&self.siaddr.as_bytes());
        buffer[24..28].copy_from_slice(&self.giaddr.as_bytes());

        buffer[28..34].copy_from_slice(&self.chaddr.as_bytes());

        // 34 -> 44 is padding for mac addr (must be 16B)
        // 44 -> 108 server address is blank
        // 108 -> 236 no boot file is blank

        buffer[236..240].copy_from_slice(&Self::MAGIC_COOKIE);

        let mut pos = 240;
        for option in &self.options {
            if option.code == 255 {
                continue;
            }

            buffer[pos] = option.code;
            pos += 1;

            buffer[pos] = option.data.len() as u8;
            pos += 1;

            buffer[pos..pos + option.data.len()].copy_from_slice(&option.data);
            pos += option.data.len();
        }

        buffer[pos] = 255;

        buffer
    }

    pub fn get_message_type(&self) -> Option<MessageType> {
        for option in &self.options {
            if option.code == 53 && !option.data.is_empty() {
                match option.data[0] {
                    1 => return Some(MessageType::Discover),
                    2 => return Some(MessageType::Offer),
                    3 => return Some(MessageType::Request),
                    4 => return Some(MessageType::Decline),
                    5 => return Some(MessageType::Ack),
                    6 => return Some(MessageType::Nak),
                    7 => return Some(MessageType::Release),
                    8 => return Some(MessageType::Inform),
                    _ => return None,
                }
            }
        }
        None
    }

    pub fn get_option(&self, code: u8) -> Option<&Vec<u8>> {
        for option in &self.options {
            if option.code == code {
                return Some(&option.data);
            }
        }
        None
    }

    pub fn get_server_identifier(&self) -> Option<Ipv4Address> {
        if let Some(data) = self.get_option(54) {
            if data.len() == 4 {
                return Ipv4Address::from_bytes(data).ok();
            }
        }
        None
    }

    pub fn get_requested_ip(&self) -> Option<Ipv4Address> {
        if let Some(data) = self.get_option(50) {
            if data.len() == 4 {
                return Ipv4Address::from_bytes(data).ok();
            }
        }
        None
    }

    pub fn get_subnet_mask(&self) -> Option<Ipv4Address> {
        if let Some(data) = self.get_option(1) {
            if data.len() == 4 {
                return Ipv4Address::from_bytes(data).ok();
            }
        }
        None
    }

    pub fn get_router(&self) -> Option<Ipv4Address> {
        if let Some(data) = self.get_option(3) {
            if data.len() == 4 {
                return Ipv4Address::from_bytes(data).ok();
            }
        }
        None
    }

    pub fn get_lease_time(&self) -> Option<u32> {
        if let Some(data) = self.get_option(51) {
            if data.len() == 4 {
                return Some(NetworkEndian::read_u32(data));
            }
        }
        None
    }

    pub fn get_dns_servers(&self) -> Vec<Ipv4Address> {
        let mut servers = Vec::new();
        if let Some(data) = self.get_option(6) {
            if data.len() % 4 == 0 {
                for i in (0..data.len()).step_by(4) {
                    if let Ok(ip) = Ipv4Address::from_bytes(&data[i..i + 4]) {
                        servers.push(ip);
                    }
                }
            }
        }
        servers
    }
}
