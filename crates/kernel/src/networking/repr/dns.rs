use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use crate::networking::{Error, Result};
use byteorder::{ByteOrder, NetworkEndian};
use crate::networking::repr::Ipv4Address;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsHeader {
    pub id: u16,
    pub flags: u16,
    pub qdcount: u16,
    pub ancount: u16,
    pub nscount: u16,
    pub arcount: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsQuestion {
    pub qname: String,
    pub qtype: u16,
    pub qclass: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsRecord {
    pub name: String,
    pub rtype: u16,
    pub rclass: u16,
    pub ttl: u32,
    pub rdata: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub header: DnsHeader,
    pub questions: Vec<DnsQuestion>,
    pub answers: Vec<DnsRecord>,
    pub authorities: Vec<DnsRecord>,
    pub additionals: Vec<DnsRecord>,
    pub ip: Ipv4Address,
}

impl Packet {
    pub fn create_dns_query(domain: &str) -> Self {
        let header = DnsHeader {
            id: 0x1337,
            flags: 0x0100,      // Standard query (QR=0), recursion desired (RD=1)
            qdcount: 1,         // One question
            ancount: 0,         // No answers initially
            nscount: 0,         // No authorities initially
            arcount: 0,         // No additionals initially
        };

        let question = DnsQuestion {
            qname: String::from(domain),
            qtype: 1,   // Type A (IPv4 address)
            qclass: 1,  // Class IN (Internet)
        };

        Packet {
            header,
            questions: vec![question],
            answers: Vec::new(),
            authorities: Vec::new(),
            additionals: Vec::new(),
            ip: Ipv4Address::empty(),
        }
    }

    pub fn extract_ip_address(&self) -> Option<Ipv4Address> {
        for record in &self.answers {
            if record.rtype == 1 && record.rdata.len() == 4 {
                let ip = Ipv4Address::new([record.rdata[0], record.rdata[1], record.rdata[2], record.rdata[3]]);
                return Some(ip);
            }
        }

        Some(self.ip)
    }


    pub fn deserialize(buffer: &[u8]) -> Result<Self> {
        if buffer.len() < 12 {
            return Err(Error::Malformed);
        }

        let id = NetworkEndian::read_u16(&buffer[0..2]);
        let flags = NetworkEndian::read_u16(&buffer[2..4]);
        let qdcount = NetworkEndian::read_u16(&buffer[4..6]);
        let ancount = NetworkEndian::read_u16(&buffer[6..8]);
        let nscount = NetworkEndian::read_u16(&buffer[8..10]);
        let arcount = NetworkEndian::read_u16(&buffer[10..12]);

        // let mut offset = 12;
        // let mut questions = Vec::new();
        // for _ in 0..qdcount {
        //     let (qname, next_offset) = read_qname(buffer, offset)?;
        //     offset = next_offset;
        //     if offset + 4 > buffer.len() {
        //         return Err(Error::Malformed);
        //     }
        //     let qtype = NetworkEndian::read_u16(&buffer[offset..offset+2]);
        //     let qclass = NetworkEndian::read_u16(&buffer[offset+2..offset+4]);
        //     offset += 4;
        //     questions.push(DnsQuestion { qname, qtype, qclass });
        // }
        //
        // let mut answers = Vec::new();
        // for _ in 0..ancount {
        //     println!("\t[!]ANSWER offset {}", offset );
        //     let (record, next_offset) = read_record(buffer, offset)?;
        //     answers.push(record);
        //     offset = next_offset;
        // }
        //
        // let mut authorities = Vec::new();
        // for _ in 0..nscount {
        //     let (record, next_offset) = read_record(buffer, offset)?;
        //     authorities.push(record);
        //     offset = next_offset;
        // }
        //
        // let mut additionals = Vec::new();
        // for _ in 0..arcount {
        //     let (record, next_offset) = read_record(buffer, offset)?;
        //     additionals.push(record);
        //     offset = next_offset;
        // }

        let len = buffer.len();
        Ok(Packet {
            header: DnsHeader { id, flags, qdcount, ancount, nscount, arcount },
            questions: Vec::new(),
            answers: Vec::new(),
            authorities: Vec::new(),
            additionals: Vec::new(),
            ip: Ipv4Address::new([buffer[len-4], buffer[len-3], buffer[len-2], buffer[len-1]]),
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        let mut header = [0u8; 12];
        NetworkEndian::write_u16(&mut header[0..2], self.header.id);
        NetworkEndian::write_u16(&mut header[2..4], self.header.flags);
        NetworkEndian::write_u16(&mut header[4..6], self.questions.len() as u16);
        NetworkEndian::write_u16(&mut header[6..8], self.answers.len() as u16);
        NetworkEndian::write_u16(&mut header[8..10], self.authorities.len() as u16);
        NetworkEndian::write_u16(&mut header[10..12], self.additionals.len() as u16);
        buffer.extend_from_slice(&header);

        for question in &self.questions {
            write_qname(&mut buffer, &question.qname);
            let mut qinfo = [0u8; 4];
            NetworkEndian::write_u16(&mut qinfo[0..2], question.qtype);
            NetworkEndian::write_u16(&mut qinfo[2..4], question.qclass);
            buffer.extend_from_slice(&qinfo);
        }

        for record in &self.answers {
            write_record(&mut buffer, record);
        }
        for record in &self.authorities {
            write_record(&mut buffer, record);
        }
        for record in &self.additionals {
            write_record(&mut buffer, record);
        }

        buffer
    }
}

fn read_qname(buffer: &[u8], mut offset: usize) -> Result<(String, usize)> {
    let mut labels = Vec::new();
    loop {
        if offset >= buffer.len() {
            return Err(Error::Malformed);
        }
        let len = buffer[offset] as usize;
        offset += 1;
        if len == 0 {
            break;
        }
        if offset + len > buffer.len() {
            return Err(Error::Malformed);
        }

        let label = core::str::from_utf8(&buffer[offset..offset+len]).map_err(|_| Error::Malformed)?;
        labels.push(String::from(label));
        offset += len;
    }
    Ok((labels.join("."), offset))
}

fn write_qname(buffer: &mut Vec<u8>, name: &str) {
    for part in name.split('.') {
        buffer.push(part.len() as u8);
        buffer.extend_from_slice(part.as_bytes());
    }
    buffer.push(0);
}

fn read_record(buffer: &[u8], offset: usize) -> Result<(DnsRecord, usize)> {
    let (name, mut pos) = read_qname(buffer, offset)?;
    if pos + 10 > buffer.len() {
    println!("malformed ");
        return Err(Error::Malformed);
    }
    let rtype = NetworkEndian::read_u16(&buffer[pos..pos+2]);
    let rclass = NetworkEndian::read_u16(&buffer[pos+2..pos+4]);
    let ttl = NetworkEndian::read_u32(&buffer[pos+4..pos+8]);
    let rdlength = NetworkEndian::read_u16(&buffer[pos+8..pos+10]) as usize;
    pos += 10;

    if pos + rdlength > buffer.len() {
    println!("malformed ");
        return Err(Error::Malformed);
    }
    let rdata = buffer[pos..pos+rdlength].to_vec();
    pos += rdlength;

    println!("made it");

    Ok((
        DnsRecord { name, rtype, rclass, ttl, rdata },
        pos,
    ))
}

fn write_record(buffer: &mut Vec<u8>, record: &DnsRecord) {
    write_qname(buffer, &record.name);
    let mut rinfo = [0u8; 10];
    NetworkEndian::write_u16(&mut rinfo[0..2], record.rtype);
    NetworkEndian::write_u16(&mut rinfo[2..4], record.rclass);
    NetworkEndian::write_u32(&mut rinfo[4..8], record.ttl);
    NetworkEndian::write_u16(&mut rinfo[8..10], record.rdata.len() as u16);
    buffer.extend_from_slice(&rinfo);
    buffer.extend_from_slice(&record.rdata);
}

