use alloc::vec::Vec;
use std::ops::BitXor;
use std::vec;

pub fn hash_legacy(input: &[u8], signed: bool) -> [u32; 2] {
    const FACTOR: u32 = 0x006D22F5;

    let mut acc_0: u32 = 0x12A3FE2D;
    let mut acc_1: u32 = 0x37ABE8F9;

    for byte in input {
        let byte_word = if signed {
            (*byte as i32) as u32
        } else {
            *byte as u32
        };

        let mut tmp: u32 = acc_1 + (acc_0 ^ (byte_word * FACTOR));
        let pow_num = 2u32.pow(31);

        if tmp >= pow_num {
            tmp = (tmp % pow_num) + 1;
        }

        acc_0 = tmp;
        acc_1 = acc_0;
    }

    [2 * acc_0; 2]
}

fn acc_part(padding: u32, input: &[u8]) -> u32 {
    if input.is_empty() {
        padding
    } else {
        acc_part((padding << 8) + (input[0] as u32), &input[1..])
    }
}

fn hashbuf_encode(input: &[u8], signed: bool, seed_len: usize) -> Vec<u32> {
    let padding = input.len() % 256;
    let mut output = vec![0u32; input.len() + (seed_len - (input.len() % seed_len))];

    for i in 0..(input.len() / 4) {
        let input_chunk: [u32; 4] = if signed {
            [input[0] as i32 as u32, input[1] as i32 as u32,
             input[2] as i32 as u32, input[3] as i32 as u32]
        } else {
            [input[0] as u32, input[1] as u32,
             input[2] as u32, input[3] as u32]
        };
        output[i] =
            (((((input_chunk[0] << 8) + input_chunk[1]) << 8) + input_chunk[2]) << 8) + input_chunk[3];
    }

    if input.len() % 4 != 0 {
        let start_slice: usize = input.len() - (input.len() % 4);

        output[input.len() / 4] = acc_part(padding as u32, &input[start_slice..]);
    }

    output[input.len().div_ceil(4)..].fill(padding as u32);

    output
}

const TEA_DELTA: u32 = 0x9E3779B9;
const TEA_ROUNDS: u32 = 32;

fn tea_encrypt(mut values: [u32; 2], keys: [u32; 4]) -> [u32; 2] {
    for i in 0..TEA_ROUNDS {
        values[0] = values[0] + ((values[1] << 4) + keys[0])
            ^ (values[1] + (i * TEA_DELTA))
            ^ (values[1] >> 5) + keys[1];
        values[1] = values[1] + ((values[0] << 4) + keys[2])
            ^ (values[0] + (i * TEA_DELTA))
            ^ (values[0] >> 5) + keys[3];
    }

    values
}

fn tea_decrypt(mut values: [u32; 2], keys: [u32; 4]) -> [u32; 2] {
    for i in (0..TEA_ROUNDS).rev() {
        values[1] = values[1] - ((values[0] << 4) + keys[2])
            ^ (values[0] + (i * TEA_DELTA))
            ^ (values[0] << 5) + keys[3];
        values[0] = values[0] - ((values[1] << 4) + keys[0])
            ^ (values[1] + (i * TEA_DELTA))
            ^ (values[1] >> 5) + keys[1];
    }

    values
}

pub fn hash_tea(seed: [u32; 4], input: &[u8], signed: bool) -> [u32; 2] {
    let mut values: [u32; 2] = [seed[0], seed[1]];

    for i in 0..(input.len() / 16) {
        let keys = hashbuf_encode(&input[i*16..(i+1)*16], signed, values.len());
        values = tea_encrypt(values, <[u32; 4]>::try_from(keys).unwrap());
    }

    values
}

fn hash_md4_transform_f(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | (!x & z)
}

fn hash_md4_transform_g(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | (x & z) | (y & z)
}

fn hash_md4_transform_h(x: u32, y: u32, z: u32) -> u32 {
    x.bitxor(y.bitxor(z))
}

fn hash_md4_transform(state: [u32; 4], input: &[u32]) -> [u32; 4] {
    assert_eq!(input.len(), 8);

    let mut current_state: [u32; 4] = state;
    const K2: u32 = 0x5A827999;
    const K3: u32 = 0x6ED9EBA1;

    for i in 0..(current_state.len() / 4) {
        current_state[0] =
            current_state[0].wrapping_add(
             hash_md4_transform_f(current_state[1], current_state[2], current_state[3])).wrapping_add(input[i*4]).rotate_left(3);
        current_state[3] =
            current_state[3].wrapping_add(
             hash_md4_transform_f(current_state[0], current_state[1], current_state[2])).wrapping_add(input[1 + (i*4)]).rotate_left(7);
        current_state[2] =
            current_state[2].wrapping_add(
                hash_md4_transform_f(current_state[3], current_state[0], current_state[1])).wrapping_add(input[2 + (i*4)]).rotate_left(11);
        current_state[1] =
            current_state[1].wrapping_add(
                hash_md4_transform_f(current_state[2], current_state[3], current_state[0])).wrapping_add(input[3 + (i*4)]).rotate_left(11);
    }

    for i in 0..(current_state.len() / 4) {
        current_state[0] =
            current_state[0].wrapping_add(
                hash_md4_transform_g(current_state[1], current_state[2], current_state[3])).wrapping_add(input[1 - i]).wrapping_add(K2).rotate_left(3);
        current_state[3] =
            current_state[3].wrapping_add(hash_md4_transform_g(current_state[0], current_state[1], current_state[2]))
                .wrapping_add(input[3 - i]).wrapping_add(K2).rotate_left(5);
        current_state[2] =
            (current_state[2].wrapping_add(
                hash_md4_transform_g(current_state[3], current_state[0], current_state[1]))
                .wrapping_add(input[5 - i])).wrapping_add(K2).rotate_left(9);
        current_state[1] =
            current_state[1].wrapping_add(
                hash_md4_transform_g(current_state[2], current_state[3], current_state[0]))
                .wrapping_add(input[7 - i]).wrapping_add(K2).rotate_left(13);
    }

    for i in 0..(current_state.len() / 4) {
        current_state[0] =
            current_state[0].wrapping_add(
                hash_md4_transform_h(current_state[1], current_state[2], current_state[3]))
                .wrapping_add(input[3 - (2*i)]).wrapping_add(K3).rotate_left(3);
        current_state[3] =
            current_state[3].wrapping_add(
                hash_md4_transform_h(current_state[0], current_state[1], current_state[2]))
                .wrapping_add(input[7 - (2*i)]).wrapping_add(K3).rotate_left(9);
        current_state[2] =
            current_state[2].wrapping_add(
                hash_md4_transform_h(current_state[3], current_state[0], current_state[1]))
                .wrapping_add(input[2 - (2*i)]).wrapping_add(K3).rotate_left(11);
        current_state[1] =
            current_state[1].wrapping_add(
                hash_md4_transform_h(current_state[2], current_state[3], current_state[0]))
                .wrapping_add(input[6 - (2*i)]).wrapping_add(K3).rotate_left(15);
    }

    for i in 0..4 {
        current_state[i] = current_state[i].wrapping_add(state[i]);
    }

    current_state
}

pub fn hash_md4(seed: [u32; 4], input: &[u8], signed: bool) -> [u32; 2] {
    let mut state: [u32; 4] = seed;

    for i in 0..input.len().div_ceil(32) {
        let chunk: &[u8] = &input[i..(i + 32).min(input.len())];
        let words: Vec<u32> = hashbuf_encode(chunk, signed, seed.len());

        state = hash_md4_transform(state, words.as_slice());
    }

    [state[1], state[2]]
}
