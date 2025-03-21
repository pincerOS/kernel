#![no_std]
#![warn(clippy::large_stack_arrays)]

// initial alex suggestions:
// want to move away from strings and use bytes
// minimize usage of vec
// get_word and get_half_word to from_bytes_le
// alloc::vec::Vec and alloc::string::String instead of std::prelude
// no intermediate vec for find() of dir entries
// make read_inode_block simpler
// read_block
// logical_block_length -> logical_block_count
// BLOCK_SIZE converting from a constant to superblock-defined (urgent)
// make a slice of the buffer and use that on logical block read
// trim_end_matches is wrong: after we read in truncate to the right length
// at the first part read blocks and then go through directory entries because ext provides a
// guranteee that dir entries wont cross boundaries--should get rid of raw bytes
// making a get_dir_entries external iteration code (check code alex sent Bobby on Discord)
// for test writing use python bytes syntax to get array of bytes instead of string
// if you see an error and dont know about it use cargo check

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use crate::block_device::BlockDeviceError;
use alloc::vec::Vec;
use std::cmp::PartialEq;
use std::collections::BTreeMap;
use std::ops::Div;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests;

pub(crate) mod bgd;
pub(crate) mod block_device;
pub(crate) mod dir;
pub(crate) mod ext;
pub(crate) mod hash;
pub(crate) mod inode;
#[cfg(feature = "std")]
pub mod linux;
pub(crate) mod superblock;

#[derive(Debug, PartialEq)]
pub enum Ext2Error {
    BlockDeviceError(BlockDeviceError),
    UnavailableINode,
    TooLongFileName,
    InvalidMode,
    FileNotFound,
    NotEnoughDeviceSpace,
    FileSizeMismatch,
    NotUtf8,
    NotADirectory,
    UnsupportedDirHashVersion,
    NotYetImplemented,
    InvalidExtentTree,
    ExtentNotFound,
}

impl From<BlockDeviceError> for Ext2Error {
    fn from(err: BlockDeviceError) -> Self {
        Ext2Error::BlockDeviceError(err)
    }
}

type DeferredWriteMap = BTreeMap<usize, Vec<u8>>;

// https://www.nongnu.org/ext2-doc/ext2.html

pub mod reserved_inodes {
    pub const EXT2_BAD_INO: u32 = 1;
    pub const EXT2_ROOT_INO: u32 = 2;
    pub const EXT2_ACL_IDX_INO: u32 = 3;
    pub const EXT2_ACL_DATA_INO: u32 = 4;
    pub const EXT2_BOOT_LOADER_INO: u32 = 5;
    pub const EXT2_UNDEL_DIR_INO: u32 = 6;
}

const UNALLOCATED_BLOCK_SLOT: u32 = 0;

// TODO(Bobby): replace this with how we get time without std
fn get_epoch_time() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize
}
