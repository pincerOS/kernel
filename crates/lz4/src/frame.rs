// https://github.com/lz4/lz4/blob/dev/doc/lz4_Frame_format.md

use crate::block::{decode_block, Lz4BlockError};
use crate::xxh::xxh32;

const LZ4_MAGIC: [u8; 4] = [0x04, 0x22, 0x4D, 0x18];

pub enum ValidateMode {
    Checksums,
    None,
}

#[derive(Debug)]
pub struct FrameHeader {
    magic: [u8; 4],
    flag: u8,
    bd: u8,
    content_size: Option<u64>,
    dict_id: Option<u32>,
    checksum: u8,
}
impl FrameHeader {
    pub fn flag_version(&self) -> u8 {
        self.flag >> 6
    }
    pub fn flag_block_indep(&self) -> bool {
        ((self.flag >> 5) & 0b1) != 0
    }
    pub fn flag_block_checksum(&self) -> bool {
        ((self.flag >> 4) & 0b1) != 0
    }
    pub fn flag_content_size(&self) -> bool {
        ((self.flag >> 3) & 0b1) != 0
    }
    pub fn flag_content_checksum(&self) -> bool {
        ((self.flag >> 2) & 0b1) != 0
    }
    pub fn flag_dict_id(&self) -> bool {
        (self.flag & 0b1) != 0
    }
    pub fn block_max_size(&self) -> usize {
        let bs = (self.bd >> 4) & 0b111;
        if bs >= 4 {
            1 << (bs * 2 + 8)
        } else {
            0
        }
    }
    pub fn content_size(&self) -> Option<u64> {
        self.content_size
    }
    pub fn dict_id(&self) -> Option<u32> {
        self.dict_id
    }
    #[must_use]
    pub fn validate(&self) -> bool {
        self.magic == LZ4_MAGIC
            && self.flag_version() == 0b01
            && ((self.bd >> 4) & 0b111) >= 4
            && (self.flag & 0b0000_0010) == 0
            && (self.bd & 0b1000_1111) == 0
    }
}

#[derive(Debug)]
pub enum Lz4Error {
    InvalidHeader,
    HeaderChecksumMismatch,
    BlockChecksumMismatch,
    ContentChecksumMismatch,
    OutOfSpace,
    OversizeBlock,
    MissingContentChecksum,
    EarlyEOF,
    InvalidBlock(Lz4BlockError),
}

fn read_le_u32(input: &[u8]) -> Option<(u32, &[u8])> {
    let (val, input) = input.split_first_chunk()?;
    Some((u32::from_le_bytes(*val), input))
}
fn read_le_u64(input: &[u8]) -> Option<(u64, &[u8])> {
    let (val, input) = input.split_first_chunk()?;
    Some((u64::from_le_bytes(*val), input))
}

pub fn read_frame(data: &[u8]) -> Result<(FrameHeader, &[u8]), Lz4Error> {
    use Lz4Error::{HeaderChecksumMismatch, InvalidHeader};

    let mut input = data;
    let magic = *input.split_first_chunk().ok_or(InvalidHeader)?.0;
    let flag = *input.get(4).ok_or(InvalidHeader)?;
    let bd = *input.get(5).ok_or(InvalidHeader)?;

    let mut header = FrameHeader {
        magic,
        flag,
        bd,
        content_size: None,
        dict_id: None,
        checksum: 0,
    };
    input = &input[6..];

    if header.flag_content_size() {
        let size;
        (size, input) = read_le_u64(input).ok_or(InvalidHeader)?;
        header.content_size = Some(size);
    }

    if header.flag_dict_id() {
        let id;
        (id, input) = read_le_u32(input).ok_or(InvalidHeader)?;
        header.dict_id = Some(id);
    }

    let header_data = &data[4..(input.as_ptr() as usize - data.as_ptr() as usize)];
    let computed = xxh32(0, header_data);

    let [checksum, ref rest @ ..] = *input else {
        return Err(InvalidHeader);
    };
    input = rest;
    header.checksum = checksum;

    if ((computed >> 8) & 0xFF) as u8 != checksum {
        return Err(HeaderChecksumMismatch);
    }
    if !header.validate() {
        return Err(InvalidHeader);
    }

    Ok((header, input))
}

pub fn decode_frames(
    header: &FrameHeader,
    mut input: &[u8],
    output: &mut [u8],
    mut cur_idx: usize,
    validate: ValidateMode,
) -> Result<usize, Lz4Error> {
    use Lz4Error::*;

    let block_checksum = header.flag_block_checksum();
    let start = cur_idx;
    let validate = matches!(validate, ValidateMode::Checksums);

    loop {
        let block_hdr;
        (block_hdr, input) = read_le_u32(input).ok_or(EarlyEOF)?;

        // END MARK
        if block_hdr == 0x0000_0000 {
            break;
        }
        // TODO: skippable frames?

        let mode = block_hdr >> 31;
        let size = (block_hdr & !(1 << 31)) as usize;

        if size > header.block_max_size() {
            return Err(OversizeBlock);
        }

        let data;
        (data, input) = input.split_at_checked(size).ok_or(EarlyEOF)?;

        if block_checksum {
            let checksum;
            (checksum, input) = read_le_u32(input).ok_or(EarlyEOF)?;

            if validate {
                let computed = xxh32(0, data);
                if checksum != computed {
                    return Err(BlockChecksumMismatch);
                }
            }
        }

        if mode == 0 {
            // mode = 0, lz4 block compressed
            cur_idx = decode_block(data, output, cur_idx).map_err(InvalidBlock)?;
        } else {
            // mode = 1, uncompressed
            let end = cur_idx.checked_add(size).ok_or(OutOfSpace)?;
            let dst = output.get_mut(cur_idx..end).ok_or(OutOfSpace)?;
            dst.copy_from_slice(data);
            cur_idx = end;
        }
    }

    if header.flag_content_checksum() {
        let checksum;
        (checksum, input) = read_le_u32(input).ok_or(MissingContentChecksum)?;

        if validate {
            let computed = xxh32(0, &output[start..cur_idx]);
            if checksum != computed {
                return Err(ContentChecksumMismatch);
            }
        }
    }

    if !input.is_empty() {
        // TODO: handle trailing content?
    }

    Ok(cur_idx)
}
