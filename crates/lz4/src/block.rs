// https://github.com/lz4/lz4/blob/dev/doc/lz4_Block_format.md

const fn read_lsic(mut input: &[u8]) -> (u32, &[u8]) {
    let mut x = 15;
    while let [n, ref rest @ ..] = *input {
        match n {
            0xFF => x += 255,
            _ => return (x + n as u32, rest),
        }
        input = rest;
    }
    (x, &[])
}

#[derive(Debug)]
pub enum Lz4BlockError {
    EarlyEOF,
    OutOfSpace,
    OffsetTooLarge,
    InvalidOffset,
}

pub fn decode_block(
    mut input: &[u8],
    output: &mut [u8],
    mut cur_idx: usize,
) -> Result<usize, Lz4BlockError> {
    use Lz4BlockError::{EarlyEOF, InvalidOffset, OffsetTooLarge, OutOfSpace};

    if cur_idx > output.len() {
        return Err(OutOfSpace);
    }

    loop {
        let token;
        (token, input) = input.split_first().ok_or(EarlyEOF)?;

        let mut literal_len = (token >> 4) as u32;
        if literal_len == 15 {
            (literal_len, input) = read_lsic(input);
        }

        let data;
        (data, input) = input
            .split_at_checked(literal_len as usize)
            .ok_or(EarlyEOF)?;

        let dst_end = cur_idx.checked_add(data.len()).ok_or(OutOfSpace)?;
        let dst = output.get_mut(cur_idx..dst_end).ok_or(OutOfSpace)?;
        dst.copy_from_slice(data);
        cur_idx = dst_end;

        if input.is_empty() {
            // Final sequence has no offset part
            return Ok(cur_idx);
        }

        let offset;
        (offset, input) = input.split_first_chunk().ok_or(EarlyEOF)?;
        let offset = u16::from_le_bytes(*offset) as u32;

        if offset == 0 {
            return Err(InvalidOffset);
        }

        let mut match_len = (token & 0b1111) as u32;
        if match_len == 15 {
            (match_len, input) = read_lsic(input);
        }
        match_len += 4;

        let copy_dst = cur_idx;
        let copy_dst_end = cur_idx.checked_add(match_len as usize).ok_or(OutOfSpace)?;

        let copy_src = cur_idx.checked_sub(offset as usize).ok_or(OffsetTooLarge)?;
        let copy_src_end = copy_src + match_len as usize;

        let slice = output.get_mut(..copy_dst_end).ok_or(OutOfSpace)?;
        let (src, dst) = slice.split_at_mut_checked(copy_dst).unwrap();

        // match_len > offset
        if copy_src_end > copy_dst {
            if offset == 1 {
                dst.fill(src[copy_src]);
            } else {
                let src = &src[copy_src..copy_dst];
                for i in 0..dst.len() {
                    dst[i] = src[i % src.len()];
                }
            }
        } else {
            let src = &src[copy_src..copy_src_end];
            dst.copy_from_slice(src);
        }

        cur_idx = copy_dst_end;
    }
}
