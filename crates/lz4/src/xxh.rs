// https://github.com/Cyan4973/xxHash/blob/release/doc/xxhash_spec.md

const PRIME32_1: u32 = 0x9E37_79B1;
const PRIME32_2: u32 = 0x85EB_CA77;
const PRIME32_3: u32 = 0xC2B2_AE3D;
const PRIME32_4: u32 = 0x27D4_EB2F;
const PRIME32_5: u32 = 0x1656_67B1;

macro_rules! wrap {
    ($lhs:tt $(+ $rhs:tt)+) => {
        (wrap!($lhs))
            $(.wrapping_add(wrap!($rhs)))+
    };
    ($lhs:tt $(- $rhs:tt)+) => {
        (wrap!($lhs))
            $(.wrapping_sub(wrap!($rhs)))+
    };
    ($lhs:tt $(* $rhs:tt)+) => {
        (wrap!($lhs))
            $(.wrapping_mul(wrap!($rhs)))+
    };
    ($lhs:tt <<< $rhs:literal) => {
        (wrap!($lhs)).rotate_left($rhs)
    };
    ($v:ident) => { $v };
    ($v:literal) => { $v };
    (($v:ident[$e:expr])) => { $v[$e] };
    (($($tt:tt)*)) => { wrap!($($tt)*) };
}

fn striped_chunks_u32_le(slice: &[u8]) -> (impl Iterator<Item = [u32; 4]> + '_, &[u8]) {
    let chunks = slice.chunks_exact(16);
    let rest = chunks.remainder();
    let iter = chunks.map(|s| {
        [&s[0..4], &s[4..8], &s[8..12], &s[12..16]]
            .map(|s| s.try_into().unwrap())
            .map(u32::from_le_bytes)
    });
    (iter, rest)
}

fn chunks_u32_le(slice: &[u8]) -> (impl Iterator<Item = u32> + '_, &[u8]) {
    let chunks = slice.chunks_exact(4);
    let rest = chunks.remainder();
    let iter = chunks.map(|c| u32::from_le_bytes(c.try_into().unwrap()));
    (iter, rest)
}

#[must_use]
pub fn xxh32(seed: u32, stream: &[u8]) -> u32 {
    let mut acc;
    let remainder;

    if stream.len() < 16 {
        acc = seed + PRIME32_5;
        remainder = stream;
    } else {
        let mut acc1 = wrap!(seed + PRIME32_1 + PRIME32_2);
        let mut acc2 = wrap!(seed + PRIME32_2);
        let mut acc3 = wrap!(seed + 0);
        let mut acc4 = wrap!(seed - PRIME32_1);

        let (iter, rest) = striped_chunks_u32_le(stream);
        for [lane1, lane2, lane3, lane4] in iter {
            acc1 = wrap!(((acc1 + (lane1 * PRIME32_2)) <<< 13) * PRIME32_1);
            acc2 = wrap!(((acc2 + (lane2 * PRIME32_2)) <<< 13) * PRIME32_1);
            acc3 = wrap!(((acc3 + (lane3 * PRIME32_2)) <<< 13) * PRIME32_1);
            acc4 = wrap!(((acc4 + (lane4 * PRIME32_2)) <<< 13) * PRIME32_1);
        }

        acc = wrap!((acc1 <<< 1) + (acc2 <<< 7) + (acc3 <<< 12) + (acc4 <<< 18));
        remainder = rest;
    }

    acc += stream.len() as u32;

    let (iter, remainder) = chunks_u32_le(remainder);
    for lane in iter {
        acc = wrap!(((acc + (lane * PRIME32_3)) <<< 17) * PRIME32_4);
    }
    for byte in remainder {
        let lane = *byte as u32;
        acc = wrap!(((acc + (lane * PRIME32_5)) <<< 11) * PRIME32_1);
    }

    acc ^= acc >> 15;
    acc = wrap!(acc * PRIME32_2);
    acc ^= acc >> 13;
    acc = wrap!(acc * PRIME32_3);
    acc ^= acc >> 16;

    acc
}

#[test]
fn test_xxh32() {
    assert_eq!(xxh32(0, &[0xde]), 0x2330_eac0);
    assert_eq!(xxh32(0, &[0xde, 0x55, 0x47, 0x7f, 0x14]), 0x1123_48ba);

    let str16 = b"\xde\x55\x47\x7f\x14\x8f\xf1\x48\x22\x3a\x40\x96\x56\xc5\xdc\xbb";
    assert_eq!(xxh32(0, str16), 0xcdf8_9609);
}
