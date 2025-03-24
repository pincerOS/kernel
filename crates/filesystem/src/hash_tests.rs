// these are specifically white-box style test cases for the dx_hash algos in hash.rs
// the ground truth hash values have been calculated through
// [debugfs'](https://www.man7.org/linux/man-pages/man8/debugfs.8.html) dx_hash command.

use crate::hash::hash_md4;

#[test]
fn halfmd4_signed_hash_test() {
    const TEST_STRINGS: [&str; 10] = ["0.txt", "1.txt", "2.txt", "10.txt", "15.txt", "50.txt",
                                      "75.txt", "abcdef.tx", "weeeee.mov", "yay.train"];
    const TEST_HASHES: [u32; 10] = 
        [0x485d21b8, 0x444eca90, 0x368cf8ca, 0xde5613f2, 0x163feb6c, 0xb661eed8, 0xf658920, 
         0x6821f2aa, 0x12bb5102, 0xb08afe50];
    const HASH_SEED: [u32; 4] = [0x5a84aca2, 0xcb430119, 0x23aa0580, 0xe772b270];
    
    for i in 0..TEST_STRINGS.len() {
        assert_eq!(hash_md4(HASH_SEED, TEST_STRINGS[i].as_bytes(),
                     true)[0], TEST_HASHES[i]);
    }
}