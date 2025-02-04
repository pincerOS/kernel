const TABLE_SIZE: usize = 4096;

struct MatchTable {
    table: [(u32, u32); TABLE_SIZE],
}

fn hash(pat: u32) -> usize {
    ((pat.wrapping_mul(0xB7D5C82D) >> 16) % TABLE_SIZE as u32) as usize
}

impl MatchTable {
    fn new() -> Self {
        MatchTable {
            table: [(0, 0xFFFFFFFF); TABLE_SIZE],
        }
    }
    fn get(&self, pattern: u32) -> Option<u32> {
        let (pat, base) = self.table[hash(pattern)];
        (pat == pattern && base != 0xFFFFFFFF).then_some(base)
    }
    fn insert(&mut self, pattern: u32, base: u32) {
        self.table[hash(pattern)] = (pattern, base);
    }
}

pub fn compress_block(input: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut idx = 0;
    let mut last_idx = 0;
    let mut cursor = 0;
    let mut table = MatchTable::new();

    // Potential speed-up options:
    // - change step size by miss count (lz4_flex steps by floor(misses/32) + 1)
    // - hash by more of the input (on 64 bit, lz4_flex hashes 40 bits (from a 64 bit read))
    // - better hash function?
    // - don't load pattern values into the hash table; just check them
    //   directly from the input

    let input_end = input.len().saturating_sub(12);
    while idx < input_end {
        let pat = u32::from_le_bytes(input[idx..][..4].try_into().unwrap());
        let found = table.get(pat);
        table.insert(pat, idx as u32);

        if let Some(base) = found.map(|b| b as usize) {
            if base < idx && idx - base <= u16::MAX as usize {
                let off = (idx - base) as u16;

                let max_backtrack = (idx - last_idx).min(base);
                let backtrack = (1..=max_backtrack)
                    .find(|i| input[base - i] != input[idx - i])
                    .map(|b| b - 1)
                    .unwrap_or(max_backtrack);

                let input_match_end = input.len().saturating_sub(5);

                let match_src = &input[base..idx];
                let target = &input[idx..input_match_end];
                let min_match_len = 4;
                let match_len = (min_match_len..target.len())
                    .find(|&i| match_src[i % match_src.len()] != target[i])
                    .unwrap_or(target.len());

                // Not required, but this can improve compression in many cases
                if match_len < 5 {
                    continue;
                }

                if backtrack > 0 {
                    let literal = &input[last_idx..idx - backtrack];
                    let match_len = match_len + backtrack;
                    cursor += emit_block(&mut output[cursor..], literal, match_len, off)?;
                } else {
                    let literal = &input[last_idx..idx];
                    cursor += emit_block(&mut output[cursor..], literal, match_len, off)?;
                }

                idx += match_len;
                last_idx = idx;

                // Semi-arbitrary offset of 2, used by the reference impl
                // idx is at least 5 bytes from the end of input due to input_match_end
                // let pat = u32::from_le_bytes(input[idx - 2..][..4].try_into().unwrap());
                // table.insert(pat, (idx - 2) as u32);
                continue;
            }
        }
        idx += 1;
    }

    let literal = &input[last_idx..input.len()];
    cursor += emit_end_block(&mut output[cursor..], literal)?;
    Some(cursor)
}

fn emit_block(output: &mut [u8], literal: &[u8], match_len: usize, offset: u16) -> Option<usize> {
    let literal_len = literal.len();
    let match_len = match_len - 4;

    let literal_div = (literal_len.saturating_sub(15)) / 255;
    let literal_mod = (literal_len.saturating_sub(15)) % 255;
    let match_div = (match_len.saturating_sub(15)) / 255;
    let match_mod = (match_len.saturating_sub(15)) % 255;

    let start = 0;
    let lit_len_start = start + 1;
    let lit_start = lit_len_start + ((literal_len >= 15) as usize + literal_div);
    let off_start = lit_start + literal_len;
    let match_len_start = off_start + 2;
    let end = match_len_start + ((match_len >= 15) as usize + match_div);

    if output.len() < end {
        return None;
    }

    let token = ((literal_len.min(15) << 4) | match_len.min(15)) as u8;
    output[start] = token;

    if literal_len >= 15 {
        output[lit_len_start..lit_start - 1].fill(255);
        output[lit_start - 1] = literal_mod as u8;
    }

    output[lit_start..off_start].copy_from_slice(literal);

    output[off_start..match_len_start].copy_from_slice(&u16::to_le_bytes(offset));

    if match_len >= 15 {
        output[match_len_start..end - 1].fill(255);
        output[end - 1] = match_mod as u8;
    }

    Some(end)
}

fn emit_end_block(output: &mut [u8], literal: &[u8]) -> Option<usize> {
    let literal_len = literal.len();
    let literal_div = (literal_len.saturating_sub(15)) / 255;
    let literal_mod = (literal_len.saturating_sub(15)) % 255;

    let start = 0;
    let lit_len_start = start + 1;
    let lit_start = lit_len_start + ((literal_len >= 15) as usize + literal_div);
    let end = lit_start + literal_len;

    if output.len() < end {
        return None;
    }

    let token = ((literal_len.min(15) << 4) | 0) as u8;
    output[start] = token;

    if literal_len >= 15 {
        output[lit_len_start..lit_start - 1].fill(255);
        output[lit_start - 1] = literal_mod as u8;
    }

    output[lit_start..end].copy_from_slice(literal);

    Some(end)
}

#[test]
fn test_compress_ratio() {
    extern crate std;
    let data = include_bytes!("../Cargo.toml");
    let mut output = std::vec![0; data.len()];

    let len = compress_block(data, &mut output).unwrap();
    let slice = &output[..len];

    std::println!("Input len: {}, output len: {}", data.len(), len);

    let mut decompressed = std::vec![0; data.len()];
    let decomp_len = crate::block::decode_block(slice, &mut decompressed, 0).unwrap();
    let decomp_slice = &decompressed[..decomp_len];

    assert!(data == decomp_slice);
}

#[test]
fn test_compress2() {
    extern crate std;
    let mut output = std::vec![0; 1 << 12];
    let data = [1, 0, 0, 1, 0, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2];

    let len = compress_block(&data, &mut output).unwrap();
    let slice = &output[..len];

    std::println!("Input len: {}, output len: {}", data.len(), len);
    std::println!("data: {:?}", slice);

    let mut decompressed = std::vec![0; data.len()];
    let decomp_len = crate::block::decode_block(slice, &mut decompressed, 0).unwrap();
    let decomp_slice = &decompressed[..decomp_len];

    assert_eq!(data, decomp_slice);
}
