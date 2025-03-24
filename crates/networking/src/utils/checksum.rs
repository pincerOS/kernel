use byteorder::{NetworkEndian, ReadBytesExt};
use std::io::Cursor;

// internet checksum based on RFC 1071
pub fn internet_checksum(buffer: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut reader = Cursor::new(buffer);

    while let Ok(word) = reader.read_u16::<NetworkEndian>() {
        sum += word as u32;
    }

    if buffer.len() % 2 != 0 {
        let last_byte = buffer[buffer.len() - 1] as u32;
        sum += last_byte << 8; 
    }

    // fold carry bits into lower 16 
    while (sum >> 16) > 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !(sum as u16)
}

