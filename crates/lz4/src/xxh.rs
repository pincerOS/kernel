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
    (($v:ident [ $e:expr ] )) => { $v[$e] };
    (($v:ident . $f:ident )) => { $v.$f };
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
    let acc;
    let remainder;

    if stream.len() < 16 {
        acc = wrap!(seed + PRIME32_5);
        remainder = stream;
    } else {
        let mut acc_arr = XXH32Hasher::init_acc(seed);
        remainder = XXH32Hasher::write_inner(&mut acc_arr, stream);
        let [acc1, acc2, acc3, acc4] = acc_arr;
        acc = wrap!((acc1 <<< 1) + (acc2 <<< 7) + (acc3 <<< 12) + (acc4 <<< 18));
    }

    let len = stream.len() as u32;
    XXH32Hasher::finalize_with(acc, len, remainder)
}

pub struct XXH32Hasher {
    seed: u32,
    acc: [u32; 4],
    buf: [u8; 16],
    buf_len: u32,
    total_len: u64,
}

impl XXH32Hasher {
    pub fn init(seed: u32) -> Self {
        Self {
            seed,
            acc: Self::init_acc(seed),
            buf: [0; 16],
            buf_len: 0,
            total_len: 0,
        }
    }

    fn init_acc(seed: u32) -> [u32; 4] {
        [
            wrap!(seed + PRIME32_1 + PRIME32_2),
            wrap!(seed + PRIME32_2),
            wrap!(seed + 0),
            wrap!(seed - PRIME32_1),
        ]
    }

    fn write_inner<'a>(acc: &mut [u32; 4], buf: &'a [u8]) -> &'a [u8] {
        let (iter, rest) = striped_chunks_u32_le(buf);
        for [lane1, lane2, lane3, lane4] in iter {
            acc[0] = wrap!((((acc[0]) + (lane1 * PRIME32_2)) <<< 13) * PRIME32_1);
            acc[1] = wrap!((((acc[1]) + (lane2 * PRIME32_2)) <<< 13) * PRIME32_1);
            acc[2] = wrap!((((acc[2]) + (lane3 * PRIME32_2)) <<< 13) * PRIME32_1);
            acc[3] = wrap!((((acc[3]) + (lane4 * PRIME32_2)) <<< 13) * PRIME32_1);
        }
        rest
    }

    fn finalize_with(acc: u32, len_trunc: u32, remainder: &[u8]) -> u32 {
        let mut acc = wrap!(acc + len_trunc);

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

    pub fn write(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);
        if self.buf_len > 0 {
            let buf_len = self.buf_len as usize;
            let amount = data.len().min(16 - buf_len);
            self.buf[buf_len..buf_len + amount].copy_from_slice(&data[..amount]);
            Self::write_inner(&mut self.acc, &self.buf);
            self.buf_len = 0;
            data = &data[amount..];
        }

        let rest = Self::write_inner(&mut self.acc, data);

        self.buf[..rest.len()].copy_from_slice(rest);
        self.buf_len = rest.len() as u32;
    }

    #[must_use]
    pub fn finalize(self) -> u32 {
        let [acc1, acc2, acc3, acc4] = self.acc;
        let mut acc = wrap!((acc1 <<< 1) + (acc2 <<< 7) + (acc3 <<< 12) + (acc4 <<< 18));

        if self.total_len < 16 {
            acc = wrap!((self.seed) + PRIME32_5);
        }

        let remainder = &self.buf[..self.buf_len as usize];
        Self::finalize_with(acc, self.total_len as u32, remainder)
    }
}

#[test]
fn test_xxh32() {
    assert_eq!(xxh32(0, &[0xde]), 0x2330_eac0);
    assert_eq!(xxh32(0, &[0xde, 0x55, 0x47, 0x7f, 0x14]), 0x1123_48ba);

    let str16 = b"\xde\x55\x47\x7f\x14\x8f\xf1\x48\x22\x3a\x40\x96\x56\xc5\xdc\xbb";
    assert_eq!(xxh32(0, str16), 0xcdf8_9609);
}
