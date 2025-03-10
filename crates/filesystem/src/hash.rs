use alloc::vec::Vec;
use std::vec;

pub fn hash_legacy(input: &[u8], signed: bool) -> u32 {
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

    2 * acc_0
}

fn acc_part(padding: u32, input: &[u8]) -> u32 {
    if input.is_empty() {
        padding
    } else {
        acc_part((padding << 8) + (input[0] as u32), &input[1..])
    }
}

/*fn hashbuf_encode(input: &[u8], signed: bool) -> Vec<u32> {
    let padding = input.len() % 256;
    let output = vec![0u32; (input.len() / 4) + 1];

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

        output[start_slice..] = acc_part(padding as u32, &input[start_slice..]);
    }

    output[input.len() / 4] = padding as u32;

    output
}*/

const TEA_DELTA: u32 = 0x9E3779B9;
const TEA_ROUNDS: u32 = 32;

fn tea_encrypt(mut values: [u32; 2], keys: [u32; 4]) -> [u32; 2] {
    for i in 0..TEA_ROUNDS {
        values[0] =
            values[0] + ((values[1] << 4) + keys[0]) ^ (values[1] + (i * TEA_DELTA))
                ^ (values[1] >> 5) + keys[1];
        values[1] = values[1] + ((values[0] << 4) + keys[2]) ^ (values[0] + (i * TEA_DELTA))
            ^ (values[0] >> 5) + keys[3];
    }

    values
}

fn tea_decrypt(mut values: [u32; 2], keys: [u32; 4]) -> [u32; 2] {
    for i in (0..TEA_ROUNDS).rev() {
        values[1] =
            values[1] - ((values[0] << 4) + keys[2]) ^ (values[0] + (i * TEA_DELTA))
                ^ (values[0] << 5) + keys[3];
        values[0] =
            values[0] - ((values[1] << 4) + keys[0]) ^ (values[1] + (i * TEA_DELTA))
                ^ (values[1] >> 5) + keys[1];
    }

    values
}

/*fn hash_tea(seed: [u32; 4], input: &[u8], signed: bool) -> [u32; 2] {
    let mut values: [u32; 2] = [seed[0], seed[1]];

    for i in 0..(input.len() / 16) {
        let keys = hashbuf_encode(&input[i*16..(i+1)*16], signed);
        values = tea_encrypt(values, <[u32; 4]>::try_from(keys).unwrap());
    }

    values
}

fn hash_md4_f() {
    //
}

fn hash_md4_g() {
    //
}

fn hash_md4_h() {
    //
}

fn hash_md4(keys: [u32; 4], input: &[u32]) {
    for i in 0..input
}*/