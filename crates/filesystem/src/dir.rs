use crate::block_device::BlockDevice;
use crate::ext::Ext;
use crate::inode::INodeWrapper;
use crate::{DeferredWriteMap, Ext2Error};

pub mod file_type {
    pub const EXT2_FT_UNKNOWN: u8 = 0;
    pub const EXT2_FT_REG_FILE: u8 = 1;
    pub const EXT2_FT_DIR: u8 = 2;
    pub const EXT2_FT_CHRDEV: u8 = 3;
    pub const EXT2_FT_BLKDEV: u8 = 4;
    pub const EXT2_FT_FIFO: u8 = 5;
    pub const EXT2_FT_SOCK: u8 = 6;
    pub const EXT2_FT_SYMLINK: u8 = 7;
}

pub mod DirectoryEntryConstants {
    pub const MAX_FILE_NAME_LEN: usize = 255;
    pub const MIN_DIRECTORY_ENTRY_SIZE: usize = 8;
}

pub mod dirhash {
    pub const LEGACY: u8 = 0x0;
    pub const HALF_MD4: u8 = 0x1;
    pub const TEA: u8 = 0x2;
    pub const LEGACY_UNSIGNED: u8 = 0x3;
    pub const HALF_MD4_UNSIGNED: u8 = 0x4;
    pub const TEA_UNSIGNED: u8 = 0x5;
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DirectoryEntryData {
    // assumption: name length <= 2^16
    // all part of the spec
    pub(crate) inode_num: u32,
    pub(crate) rec_len: u16,
    pub(crate) name_len: u8,
    pub(crate) file_type: u8,

    pub(crate) name_characters: [u8; 256],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HTreeDirectoryEntryRoot {
    dx_root_reserved: u32,
    pub(crate) hash_version: u8,
    info_length: u8,
    pub(crate) indirect_levels: u8,
    unused_flags: u8,

    limit: u16,
    pub(crate) count: u16,
    pub(crate) block: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HTreeDirectoryEntryNode {
    limit: u16,
    pub(crate) count: u16,
    pub(crate) block: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HTreeDirectoryEntry {
    pub(crate) hash: u32,
    pub(crate) block: u32,
}

//const _: () = assert!(size_of::<HTreeDirectoryEntryRoot>() == 128);
const _: () = assert!(size_of::<HTreeDirectoryEntry>() == 8);

pub(crate) struct DirectoryEntryWrapper {
    pub(crate) entry: DirectoryEntryData,
    pub(crate) inode_block_num: usize,
    pub(crate) offset: usize,
}

impl DirectoryEntryWrapper {
    pub fn copy_to_bytes_slice(&self, slice: &mut [u8], use_offset: bool) {
        let entry_bytes: &[u8] = bytemuck::bytes_of(&self.entry);

        // we shouldn't write self.entry.rec_len because that contains padding bytes
        // which can be larger than the maximum directory entry size without padding
        let dir_rec_len_to_write =
            (self.entry.name_len as usize) + DirectoryEntryConstants::MIN_DIRECTORY_ENTRY_SIZE;
        let entry_slice: &[u8] = &entry_bytes[0..dir_rec_len_to_write];

        if use_offset {
            slice[self.offset..(self.offset + dir_rec_len_to_write)].copy_from_slice(entry_slice);
        } else {
            slice.copy_from_slice(entry_slice);
        }
    }

    pub fn add_deferred_write<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        dir_node: &mut INodeWrapper,
        deferred_writes: &mut DeferredWriteMap,
    ) -> Result<(), Ext2Error> {
        let entry_bytes: &[u8] = bytemuck::bytes_of(&self.entry);

        // we shouldn't write self.entry.rec_len because that contains padding bytes
        // which can be larger than the maximum directory entry size without padding
        let dir_rec_len_to_write =
            (self.entry.name_len as usize) + DirectoryEntryConstants::MIN_DIRECTORY_ENTRY_SIZE;
        let entry_slice: &[u8] = &entry_bytes[0..dir_rec_len_to_write];

        let block_num =
            dir_node.get_inode_block_num(self.inode_block_num, ext2, Some(deferred_writes))?
                as usize;

        ext2.add_write_to_deferred_writes_map(
            deferred_writes,
            block_num,
            self.offset,
            entry_slice,
            None,
        )
    }
}
