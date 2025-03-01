// https://github.com/lz4/lz4/blob/dev/doc/lz4_Frame_format.md

use crate::block::{decode_block, Lz4BlockError};
use crate::xxh::{xxh32, XXH32Hasher};

const LZ4_MAGIC: [u8; 4] = [0x04, 0x22, 0x4D, 0x18];

#[derive(Copy, Clone, PartialEq)]
pub enum ValidateMode {
    Checksums,
    None,
}

pub struct FrameOptions {
    pub block_indep: bool,
    pub block_checksum: bool,
    pub content_checksum: bool,
    pub content_size: ContentSize,
    pub max_block_size: MaxBlockSize,
}

impl FrameOptions {
    pub fn max_compressed_size(&self, data_len: usize) -> usize {
        let has_content_size = !matches!(self.content_size, ContentSize::None);
        let has_dict_id = false;
        let header_size = 7 + (has_content_size as usize * 8) + (has_dict_id as usize * 4);

        let max_num_blocks = data_len.div_ceil(self.max_block_size.size());
        let block_meta = max_num_blocks * (4 + (self.block_checksum as usize) * 4);
        let trailer = 4 + (self.content_checksum as usize) * 4;

        data_len + header_size + block_meta + trailer
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum ContentSize {
    None,
    Detect,
    Known(u64),
}

#[derive(Copy, Clone, PartialEq)]
pub enum MaxBlockSize {
    Size64KiB,
    Size256KiB,
    Size1MiB,
    Size4MiB,
}

impl MaxBlockSize {
    pub fn size(&self) -> usize {
        match self {
            MaxBlockSize::Size64KiB => 1 << 16,
            MaxBlockSize::Size256KiB => 1 << 18,
            MaxBlockSize::Size1MiB => 1 << 20,
            MaxBlockSize::Size4MiB => 1 << 22,
        }
    }
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
    pub fn new(opt: &FrameOptions) -> Self {
        let has_content_size = !matches!(opt.content_size, ContentSize::None);
        let has_dict = false;
        let block_size_idx = match opt.max_block_size {
            MaxBlockSize::Size64KiB => 0b100,
            MaxBlockSize::Size256KiB => 0b101,
            MaxBlockSize::Size1MiB => 0b110,
            MaxBlockSize::Size4MiB => 0b111,
        };
        FrameHeader {
            magic: super::frame::LZ4_MAGIC,
            flag: (0b01 << 6)
                | ((opt.block_indep as u8) << 5)
                | ((opt.block_checksum as u8) << 4)
                | ((has_content_size as u8) << 3)
                | ((opt.content_checksum as u8) << 2)
                | (has_dict as u8),
            bd: (block_size_idx << 4),
            content_size: match opt.content_size {
                ContentSize::Known(s) => Some(s),
                _ => None,
            },
            dict_id: None,
            checksum: 0,
        }
    }

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

pub(crate) fn read_le_u32(input: &[u8]) -> Option<(u32, &[u8])> {
    let (val, input) = input.split_first_chunk()?;
    Some((u32::from_le_bytes(*val), input))
}
pub(crate) fn read_le_u64(input: &[u8]) -> Option<(u64, &[u8])> {
    let (val, input) = input.split_first_chunk()?;
    Some((u64::from_le_bytes(*val), input))
}
pub(crate) fn write_le_u32(out: &mut [u8], value: u32) -> Option<&mut [u8]> {
    let (val, out) = out.split_first_chunk_mut::<4>()?;
    val.copy_from_slice(&u32::to_le_bytes(value));
    Some(out)
}
pub(crate) fn write_le_u64(out: &mut [u8], value: u64) -> Option<&mut [u8]> {
    let (val, out) = out.split_first_chunk_mut::<8>()?;
    val.copy_from_slice(&u64::to_le_bytes(value));
    Some(out)
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
        // not required by the spec, but lz4 treats concatenated
        // frames as a single compressed file.
    }

    Ok(cur_idx)
}

#[derive(Debug)]
pub enum CompressError {
    BufferTooSmall,
    InvalidHeader,
}

pub fn write_header(data: &mut [u8], header: &FrameHeader) -> Result<usize, CompressError> {
    let size = 7 + (header.flag_content_size() as usize * 8) + (header.flag_dict_id() as usize * 4);
    if data.len() < size {
        return Err(CompressError::BufferTooSmall);
    }
    let mut out = &mut *data;
    out[..4].copy_from_slice(&header.magic);
    out[4] = header.flag;
    out[5] = header.bd;
    out = &mut out[6..];

    if header.flag_content_size() {
        let size = header.content_size.unwrap_or(0);
        out = write_le_u64(out, size).unwrap();
    }
    if header.flag_dict_id() {
        let id = header.dict_id.ok_or(CompressError::InvalidHeader)?;
        out = write_le_u32(out, id).unwrap();
    }
    let _ = out;

    let hash = xxh32(0, &data[4..size - 1]);
    data[size - 1] = ((hash >> 8) & 0xFF) as u8;

    Ok(size)
}

// TODO: streaming compression, rather than in-place
pub fn encode_frames(
    header: &FrameOptions,
    mut input: &[u8],
    output: &mut [u8],
    mut cur_idx: usize,
) -> Result<usize, CompressError> {
    use crate::compress::{compress_block, MatchTable};

    let block_checksum = header.block_checksum;
    let content_checksum = header.content_checksum;
    let mut hasher = XXH32Hasher::init(0);

    // TODO: move this to the heap, to avoid overflowing the stack
    let mut table_slot = core::mem::MaybeUninit::uninit();
    let table = MatchTable::new_in_place(&mut table_slot);

    while !input.is_empty() {
        // block header (to fill in later)
        let header_idx = cur_idx;
        write_le_u32(&mut output[header_idx..], 0x0000_0000)
            .ok_or(CompressError::BufferTooSmall)?;
        cur_idx += 4;

        let block_size = header.max_block_size.size();
        let input_part = &input[..block_size.min(input.len())];
        let output_range = output[cur_idx..]
            .get_mut(..input_part.len())
            .ok_or(CompressError::BufferTooSmall)?;

        let size = compress_block(input_part, output_range, table);

        let header;
        if let Some(size) = size {
            header = size as u32;
            cur_idx += size;
        } else {
            // Not enough space to compress; store uncompressed
            header = input_part.len() as u32 | (1 << 31);
            output_range.copy_from_slice(input_part);
            cur_idx += input_part.len();
        }

        write_le_u32(&mut output[header_idx..], header).ok_or(CompressError::BufferTooSmall)?;

        input = &input[input_part.len()..];

        if content_checksum {
            hasher.write(input_part);
        }

        if block_checksum {
            let checksum = xxh32(0, &output[header_idx + 4..cur_idx]);
            write_le_u32(&mut output[cur_idx..], checksum).ok_or(CompressError::BufferTooSmall)?;
            cur_idx += 4;
        }
    }

    // end marker
    write_le_u32(&mut output[cur_idx..], 0x0000_0000).ok_or(CompressError::BufferTooSmall)?;
    cur_idx += 4;

    if header.content_checksum {
        let checksum = hasher.finalize();
        write_le_u32(&mut output[cur_idx..], checksum).ok_or(CompressError::BufferTooSmall)?;
        cur_idx += 4;
    }

    Ok(cur_idx)
}
