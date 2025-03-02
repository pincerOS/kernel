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

use alloc::borrow::Cow;
use std::cell::RefCell;
use std::cmp::PartialEq;
use std::collections::BTreeMap;
use std::{print, slice};
use std::ptr::{read, write};
use std::time::{SystemTime, UNIX_EPOCH};
use bytemuck::bytes_of;
use crate::Ext2Error::NotEnoughDeviceSpace;
use crate::i_mode::{EXT2_S_IFDIR, EXT2_S_IFREG};
use crate::linux::FileBlockDevice;
use alloc::rc::{Rc, Weak};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::ControlFlow;
use std::ops::Div;

#[cfg(test)]
mod tests;

#[cfg(feature = "std")]
pub mod linux;

pub const SECTOR_SIZE: usize = 512;

#[derive(Debug, PartialEq)]
pub enum BlockDeviceError {
    Unknown,
}

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
}

impl From<BlockDeviceError> for Ext2Error {
    fn from(err: BlockDeviceError) -> Self {
        Ext2Error::BlockDeviceError(err)
    }
}

pub trait BlockDevice {
    fn read_sector(
        &mut self,
        index: u64,
        buffer: &mut [u8; SECTOR_SIZE],
    ) -> Result<(), BlockDeviceError>;

    fn write_sector(
        &mut self,
        index: u64,
        buffer: &[u8; SECTOR_SIZE],
    ) -> Result<(), BlockDeviceError>;

    fn read_sectors(
        &mut self,
        start_index: u64,
        buffer: &mut [u8],
    ) -> Result<(), BlockDeviceError> {
        assert!(buffer.len() % SECTOR_SIZE == 0);
        for (buf_segment, sector) in buffer.chunks_exact_mut(SECTOR_SIZE).zip(start_index..) {
            let array: &mut [u8; SECTOR_SIZE] = buf_segment.try_into().unwrap();
            self.read_sector(sector, array)?;
        }
        Ok(())
    }
    
    fn write_sectors(
        &mut self,
        start_index: u64,
        sectors: usize,
        buffer: &[u8],
    ) -> Result<(), BlockDeviceError> {
        let mut tmp_buf: [u8; SECTOR_SIZE] = [0; SECTOR_SIZE];
        for i in 0..sectors {
            let cur_sector = start_index + (i as u64);
            for j in 0..SECTOR_SIZE {
                tmp_buf[j] = buffer[(i*SECTOR_SIZE)+j];
            }
            self.write_sector(cur_sector, &tmp_buf)?;
        }
        Ok(())
    }
}

pub struct Ext2<Device> {
    device: Device,
    superblock: Superblock,
    block_group_descriptor_tables: Vec<BGD>,
    root_inode: Rc<RefCell<INodeWrapper>>,
    inode_map: BTreeMap<usize, Weak<RefCell<INodeWrapper>>>
}

type DeferredWriteMap = BTreeMap<usize, Vec<u8>>;

mod DirectoryEntryConstants {
    pub const MAX_FILE_NAME_LEN: usize = 255;
    pub const MIN_DIRECTORY_ENTRY_SIZE: usize = 8;
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DirectoryEntryData {
    // assumption: name length <= 2^16
    // all part of the spec
    inode_num: u32,
    rec_len: u16,
    name_len: u8,
    file_type: u8,

    name_characters: [u8; 256]
}
struct DirectoryEntryWrapper {
    entry: DirectoryEntryData,
    inode_block_num: usize,
    offset: usize
}

// https://www.nongnu.org/ext2-doc/ext2.html

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Superblock {
    s_inodes_count: u32,
    s_blocks_count: u32,
    s_r_blocks_count: u32,
    s_free_blocks_count: u32,
    s_free_inodes_count: u32,
    s_first_data_block: u32,
    s_log_block_size: u32,
    s_log_frag_size: u32,
    s_blocks_per_group: u32,
    s_frags_per_group: u32,
    s_inodes_per_group: u32,
    s_mtime: u32,
    s_wtime: u32,
    s_mnt_count: u16,
    s_max_mnt_count: u16,
    s_magic: u16,
    s_state: u16,
    s_errors: u16,
    s_minor_rev_level: u16,
    s_lastcheck: u32,
    s_checkinterval: u32,
    s_creator_os: u32,
    s_rev_level: u32,
    s_def_resuid: u16,
    s_def_resgid: u16,
    s_first_ino: u32,
    s_inode_size: u16,
    s_block_group_nr: u16,
    s_feature_compat: u32,
    s_feature_incompat: u32,
    s_feature_ro_compat: u32,
    s_uuid: [u8; 16],
    s_volume_name: [u8; 16],
    s_last_mounted: [u8; 64],
    s_algo_bitmap: u32,
    s_prealloc_blocks: u8,
    s_prealloc_dir_blocks: u8,
    unused_alignment_1: [u8; 2],
    s_journal_uuid: [u8; 16],
    s_journal_inum: u32,
    s_journal_dev: u32,
    s_last_orphan: u32,
    s_hash_seed: [u32; 4],
    s_def_hash_version: u8,
    unused_alignment_2: [u8; 3],
    s_default_mount_options: u32,
    s_first_meta_bg: u32,
    // for some reason (padding?) unused_alignment_4: [u8; 760] still causes
    // issues with bytemuck::Zeroable so I did this garbage instead
    unused_alignment_4: [u8; 512],
    unused_alignment_5: [u8; 128],
    unused_alignment_6: [u8; 64],
    unused_alignment_7: [u8; 32],
    unused_alignment_8: [u8; 16],
    unused_alignment_9: [u8; 8]
}

impl Superblock {
    fn get_num_of_block_groups(&self) -> u32 {
        self.s_inodes_count / self.s_inodes_per_group
    }

    fn get_block_size(&self) -> usize {
        1024 << self.s_log_block_size
    }
}

pub mod s_state {
    pub const EXT2_VALID_FS: u16 = 1;
    pub const EXT2_ERROR_FS: u16 = 2;
}

pub mod s_errors {
    pub const EXT2_ERRORS_CONTINUE: u16 = 1;
    pub const EXT2_ERRORS_RO: u16 = 2;
    pub const EXT2_ERRORS_PANIC: u16 = 3;
}

pub mod s_creator_os {
    pub const EXT2_OS_LINUX: u32 = 0;
    pub const EXT2_OS_HURD: u32 = 1;
    pub const EXT2_OS_MASIX: u32 = 2;
    pub const EXT2_OS_FREEBSD: u32 = 3;
    pub const EXT2_OS_LITES: u32 = 4;
}

pub mod s_rev_level {
    pub const EXT2_GOOD_OLD_REV: u32 = 0;
    pub const EXT2_DYNAMIC_REV: u32 = 1;
}

pub mod s_feature_compat {
    pub const EXT2_FEATURE_COMPAT_DIR_PREALLOC: u32 = 0x0001;
    pub const EXT2_FEATURE_COMPAT_IMAGIC_INODES: u32 = 0x0002;
    pub const EXT3_FEATURE_COMPAT_HAS_JOURNAL: u32 = 0x0004;
    pub const EXT2_FEATURE_COMPAT_EXT_ATTR: u32 = 0x0008;
    pub const EXT2_FEATURE_COMPAT_RESIZE_INO: u32 = 0x0010;
    pub const EXT2_FEATURE_COMPAT_DIR_INDEX: u32 = 0x0020;
}

pub mod s_feature_incompat {
    pub const EXT2_FEATURE_INCOMPAT_COMPRESSION: u32 = 0x0001;
    pub const EXT2_FEATURE_INCOMPAT_FILETYPE: u32 = 0x0002;
    pub const EXT3_FEATURE_INCOMPAT_RECOVER: u32 = 0x0004;
    pub const EXT3_FEATURE_INCOMPAT_JOURNAL_DEV: u32 = 0x0008;
    pub const EXT2_FEATURE_INCOMPAT_META_BG: u32 = 0x0010;
}

pub mod s_feature_ro_compat {
    pub const EXT2_FEATURE_RO_COMPAT_SPARSE_SUPER: u32 = 0x0001;
    pub const EXT2_FEATURE_RO_COMPAT_LARGE_FILE: u32 = 0x0002;
    pub const EXT2_FEATURE_RO_COMPAT_BTREE_DIR: u32 = 0x0004;
}

pub mod s_algo_bitmap {
    pub const EXT2_LZV1_ALG: u32 = 0x0001;
    pub const EXT2_LZRW3A_ALG: u32 = 0x0002;
    pub const EXT2_GZIP_ALG: u32 = 0x0004;
    pub const EXT2_BZIP2_ALG: u32 = 0x0008;
    pub const EXT2_LZO_ALG: u32 = 0x0010;
}

const _: () = assert!(size_of::<Superblock>() == 1024);

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BGD {
    bg_block_bitmap: u32,
    bg_inode_bitmap: u32,
    bg_inode_table: u32,
    bg_free_blocks_count: u16,
    bg_free_inodes_count: u16,
    bg_used_dirs_count: u16,
    bg_pad: u16,
    bg_reserved: [u8; 12]
}

const _: () = assert!(size_of::<BGD>() == 32);

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct INode {
    i_mode: u16,
    i_uid: u16,
    i_size: u32,
    i_atime: u32,
    i_ctime: u32,
    i_mtime: u32,
    i_dtime: u32,
    i_gid: u16,
    i_links_count: u16,
    i_blocks: u32,
    i_flags: u32,
    i_osd1: u32,
    i_block: [u32; 15], // 12 direct, single, double, triple
    i_generation: u32,
    i_file_acl: u32,
    i_dir_acl: u32,
    i_faddr: u32,
    i_osd2: [u8; 12],
}

struct INodeBlockInfo {
    block_num: usize,
    block_offset: usize,
}

pub mod reserved_inodes {
    pub const EXT2_BAD_INO: u32 = 1;
    pub const EXT2_ROOT_INO: u32 = 2;
    pub const EXT2_ACL_IDX_INO: u32 = 3;
    pub const EXT2_ACL_DATA_INO: u32 = 4;
    pub const EXT2_BOOT_LOADER_INO: u32 = 5;
    pub const EXT2_UNDEL_DIR_INO: u32 = 6;
}

pub mod i_mode {
    pub const EXT2_S_IFSOCK: u16 = 0xC000;
    pub const EXT2_S_IFLNK: u16 = 0xA000;
    pub const EXT2_S_IFREG: u16 = 0x8000;
    pub const EXT2_S_IFBLK: u16 = 0x6000;
    pub const EXT2_S_IFDIR: u16 = 0x4000;
    pub const EXT2_S_IFCHR: u16 = 0x2000;
    pub const EXT2_S_IFIFO: u16 = 0x1000;
    pub const EXT2_S_ISUID: u16 = 0x0800;
    pub const EXT2_S_ISGID: u16 = 0x0400;
    pub const EXT2_S_ISVTX: u16 = 0x0200;
    pub const EXT2_S_IRUSR: u16 = 0x0100;
    pub const EXT2_S_IWUSR: u16 = 0x0080;
    pub const EXT2_S_IXUSR: u16 = 0x0040;
    pub const EXT2_S_IRGRP: u16 = 0x0020;
    pub const EXT2_S_IWGRP: u16 = 0x0010;
    pub const EXT2_S_IXGRP: u16 = 0x0008;
    pub const EXT2_S_IROTH: u16 = 0x0004;
    pub const EXT2_S_IWOTH: u16 = 0x0002;
    pub const EXT2_S_IXOTH: u16 = 0x0001;
}

pub mod i_flags {
    pub const EXT2_SECRM_FL: u32 = 0x00000001;
    pub const EXT2_UNRM_FL: u32 = 0x00000002;
    pub const EXT2_COMPR_FL: u32 = 0x00000004;
    pub const EXT2_SYNC_FL: u32 = 0x00000008;
    pub const EXT2_IMMUTABLE_FL: u32 = 0x00000010;
    pub const EXT2_APPEND_FL: u32 = 0x00000020;
    pub const EXT2_NODUMP_FL: u32 = 0x00000040;
    pub const EXT2_NOATIME_FL: u32 = 0x00000080;
    pub const EXT2_DIRTY_FL: u32 = 0x00000100;
    pub const EXT2_COMPRBLK_FL: u32 = 0x00000200;
    pub const EXT2_NOCOMPR_FL: u32 = 0x00000400;
    pub const EXT2_ECOMPR_FL: u32 = 0x00000800;
    pub const EXT2_BTREE_FL: u32 = 0x00001000;
    pub const EXT2_INDEX_FL: u32 = 0x00001000;
    pub const EXT2_IMAGIC_FL: u32 = 0x00002000;
    pub const EXT3_JOURNAL_DATA_FL: u32 = 0x00004000;
    pub const EXT2_RESERVED_FL: u32 = 0x80000000;
}

const _: () = assert!(size_of::<INode>() == 128);

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

const UNALLOCATED_BLOCK_SLOT: u32 = 0;

#[derive(Debug)]
pub struct INodeWrapper {
    inode: INode,
    _inode_num: u32
}

// TODO(Bobby): replace this with how we get time without std
fn get_epoch_time() -> usize {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as usize
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
    
    pub fn add_deferred_write<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>,
                                              dir_node: &mut INodeWrapper,
                                              deferred_writes: &mut DeferredWriteMap)
        -> Result<(), Ext2Error> {
        let entry_bytes: &[u8] = bytemuck::bytes_of(&self.entry);
        
        // we shouldn't write self.entry.rec_len because that contains padding bytes
        // which can be larger than the maximum directory entry size without padding
        let dir_rec_len_to_write =
            (self.entry.name_len as usize) + DirectoryEntryConstants::MIN_DIRECTORY_ENTRY_SIZE ;
        let entry_slice: &[u8] = &entry_bytes[0..dir_rec_len_to_write];

        let block_num = dir_node.get_inode_block_num(self.inode_block_num, ext2,
                                                     Some(deferred_writes))? as usize;

        ext2.add_write_to_deferred_writes_map(deferred_writes, block_num, self.offset,
                                              entry_slice, None)
    }
}

impl<D> Ext2<D>
where
    D: BlockDevice,
{
    fn read_logical_block_inner(device: &mut D, superblock: &Superblock, logical_block_start: usize,
                                buffer: &mut [u8],
                                deferred_writes: Option<&DeferredWriteMap>) -> Result<(), Ext2Error> {
        assert_eq!(buffer.len(), superblock.get_block_size());
        let start_sector_numerator: usize = logical_block_start * superblock.get_block_size();
        let start_sector: usize = start_sector_numerator / SECTOR_SIZE;

        if deferred_writes.is_some() {
            let deferred_writes_unwrapped: &DeferredWriteMap = deferred_writes.unwrap();

            if deferred_writes_unwrapped.contains_key(&logical_block_start) {
                buffer[0..superblock.get_block_size()].copy_from_slice(
                    &deferred_writes_unwrapped[&logical_block_start]);
            }
        } else {
            device.read_sectors(start_sector as u64, buffer)?;
        }

        Ok(())
    }

    pub fn read_logical_block(
        &mut self,
        logical_block_start: usize,
        buffer: &mut [u8],
        deferred_writes: Option<&DeferredWriteMap>,
    ) -> Result<(), Ext2Error> {
        Self::read_logical_block_inner(
            &mut self.device,
            &self.superblock,
            logical_block_start,
            buffer,
            deferred_writes,
        )
    }

    fn write_logical_block_inner(device: &mut D, superblock: &Superblock,
                                 logical_block_start: usize, buffer: &[u8]) -> Result<(), Ext2Error> {
        assert_eq!(buffer.len(), superblock.get_block_size());

        let start_sector_numerator: usize = logical_block_start * superblock.get_block_size();
        let start_sector: usize = start_sector_numerator / SECTOR_SIZE;
        let sectors: usize = superblock.get_block_size() / SECTOR_SIZE;

        let write_result: Result<(), BlockDeviceError> =
            device.write_sectors(start_sector as u64, sectors, buffer);

        if write_result.is_ok() {
            Ok(())
        } else {
            Err(Ext2Error::BlockDeviceError(write_result.unwrap_err()))
        }
    }

    pub fn write_logical_block(&mut self, logical_block_start: usize,
                               buffer: &[u8]) -> Result<(), Ext2Error> {
        Self::write_logical_block_inner(&mut self.device, &self.superblock, logical_block_start,
                                        buffer)
    }

    fn read_logical_blocks_inner(device: &mut D, superblock: &Superblock,
                                 logical_block_start: usize, buffer: &mut [u8],
                                 deferred_writes: Option<&DeferredWriteMap>) -> Result<(), Ext2Error> {
        assert_eq!(buffer.len() % superblock.get_block_size(), 0);
        let logical_block_length = buffer.len() / superblock.get_block_size();

        for i in 0..logical_block_length {
            let slice_start: usize = i * superblock.get_block_size();
            let slice_end: usize = slice_start + superblock.get_block_size();

            Self::read_logical_block_inner(device, superblock, logical_block_start+i,
                                     &mut buffer[slice_start..slice_end], deferred_writes)?
        }

        Ok(())
    }

    fn write_logical_blocks_inner(device: &mut D, superblock: &Superblock,
                                  logical_block_start: usize, buffer: &[u8]) -> Result<(), Ext2Error> {
        assert_eq!(buffer.len() % superblock.get_block_size(), 0);
        let logical_block_length = buffer.len() / superblock.get_block_size();

        for i in 0..logical_block_length {
            let slice_start: usize = i * superblock.get_block_size();
            let slice_end: usize = slice_start + superblock.get_block_size();

            Self::write_logical_block_inner(device, superblock, logical_block_start+i,
                                            &buffer[slice_start..slice_end])?
        }

        Ok(())
    }

    fn read_logical_blocks(&mut self, logical_block_start: usize, buffer: &mut [u8],
                           deferred_writes: Option<&DeferredWriteMap>) -> Result<(), Ext2Error> {
        Self::read_logical_block_inner(&mut self.device, &self.superblock, logical_block_start,
                                       buffer, deferred_writes)
    }

    fn write_logical_blocks(&mut self, logical_block_start: usize, buffer: &mut [u8])
        -> Result<(), Ext2Error> {
        Self::write_logical_block_inner(&mut self.device, &self.superblock, logical_block_start,
                                        buffer)
    }

    fn get_block_that_has_inode(device: &mut D, superblock: &Superblock,
                       block_group_descriptor_tables: &Vec<BGD>,
                       inode_num: usize) -> INodeBlockInfo {
        let inode_size = superblock.s_inode_size as usize;

        let block_group_number = (inode_num - 1) / superblock.s_inodes_per_group as usize;
        let inode_table_block =
            block_group_descriptor_tables[block_group_number].bg_inode_table as usize;

        let inode_table_index: usize = (inode_num - 1) % (superblock.s_inodes_per_group as usize);
        let inode_table_block_with_offset: usize =
            ((inode_table_index * inode_size) / superblock.get_block_size()) + inode_table_block;
        let inode_table_interblock_offset: usize =
            (inode_table_index * inode_size) % superblock.get_block_size();

        INodeBlockInfo{
            block_num: inode_table_block_with_offset,
            block_offset: inode_table_interblock_offset
        }
    }

    fn get_inode(device: &mut D, superblock: &Superblock, block_group_descriptor_tables: &Vec<BGD>,
                 inode_num: usize, deferred_writes: Option<&DeferredWriteMap>) -> Result<INode, Ext2Error> {
        let inode_block_info: INodeBlockInfo =
            Ext2::get_block_that_has_inode(device, superblock, block_group_descriptor_tables, inode_num);
        let mut block_buffer: Vec<u8> = vec![0; superblock.get_block_size()];

        Self::read_logical_block_inner(device, superblock, inode_block_info.block_num,
                                       block_buffer.as_mut_slice(), deferred_writes)?;

        let mut inode_data: [u8; size_of::<INode>()] = [0x00; size_of::<INode>()];

        inode_data.copy_from_slice(
            &block_buffer[inode_block_info.block_offset..inode_block_info.block_offset + size_of::<INode>()]);

        let inode: INode =
            unsafe { core::mem::transmute::<[u8; size_of::<INode>()], INode>(inode_data) };

        Ok(inode)
    }

    pub fn get_inode_self(&mut self, inode_num: usize,
                          deferred_writes: Option<&DeferredWriteMap>) -> Result<INode, Ext2Error> {
        Self::get_inode(&mut self.device, &self.superblock, &self.block_group_descriptor_tables,
                        inode_num, deferred_writes)
    }

    pub fn get_root_inode_wrapper(&mut self) -> Rc<RefCell<INodeWrapper>> {
        self.root_inode.clone()
    }

    pub fn add_block_group_deferred_write(&mut self,
                                          deferred_write_map: &mut DeferredWriteMap,
                                          block_group_num: usize) -> Result<(), Ext2Error> {
        let block_size: usize = self.superblock.get_block_size();
        let block_group_descriptor_block: usize =
            if block_size == 1024 {2} else {1} + ((block_group_num * size_of::<BGD>()) / block_size);
        let block_group_descriptor_offset: usize =
            (block_group_num * size_of::<BGD>()) % block_size;

        let mut block_group_copy: [u8; size_of::<BGD>()] = [0; size_of::<BGD>()];
        {
            let block_group_as_bytes =
                bytemuck::bytes_of(&self.block_group_descriptor_tables[block_group_num]);

            block_group_copy.copy_from_slice(block_group_as_bytes);
        }

        self.add_write_to_deferred_writes_map(deferred_write_map,
                                              block_group_descriptor_block,
                                              block_group_descriptor_offset,
                                              &block_group_copy, None)?;

        Ok(())
    }

    pub fn add_super_block_deferred_write(&mut self,
                                          deferred_write_map: &mut DeferredWriteMap) -> Result<(), Ext2Error> {
        let mut superblock_bytes_copy: [u8; size_of::<Superblock>()] = [0; size_of::<Superblock>()];
        {
            let superblock_as_bytes = bytes_of(&self.superblock);

            superblock_bytes_copy.copy_from_slice(superblock_as_bytes);
        }

        // CHANGE BLOCK_NUM WHEN BLOCK_SIZE changes
        self.add_write_to_deferred_writes_map(deferred_write_map, 1, 0,
                                              &superblock_bytes_copy, None)?;

        Ok(())
    }

    pub fn new(mut device: D) -> Result<Self, Ext2Error> {
        // TODO: avoid putting this buffer on the stack, and avoid storing
        // superblock padding in the Ext2 struct
        let mut buffer = [0; 1024];

        device.read_sectors(2, &mut buffer)?;

        let superblock: Superblock =
            unsafe { core::mem::transmute::<[u8; 1024], Superblock>(buffer) };
        let block_size = superblock.get_block_size();

        let block_group_descriptor_block: usize = if block_size == 1024 { 2 } else { 1 };

        let descriptor_count = superblock.get_num_of_block_groups() as usize;
        let block_group_descriptor_blocks: usize =
            1 + (descriptor_count * size_of::<BGD>()).div_ceil(block_size);

        let max_block_group_descriptors: usize =
            block_group_descriptor_blocks * (block_size / size_of::<BGD>());

        let mut block_group_descriptor_tables: Vec<BGD> = Vec::new();

        block_group_descriptor_tables.resize(
            max_block_group_descriptors,
            BGD {
                bg_block_bitmap: 0,
                bg_inode_bitmap: 0,
                bg_inode_table: 0,
                bg_free_blocks_count: 0,
                bg_free_inodes_count: 0,
                bg_used_dirs_count: 0,
                bg_pad: 0,
                bg_reserved: [0; 12],
            },
        );

        let descriptor_table_bytes_ptr: *mut u8 =
            block_group_descriptor_tables.as_mut_ptr() as *mut u8;
        let descriptor_table_bytes_slice: &mut [u8] = unsafe {
            core::slice::from_raw_parts_mut(
                descriptor_table_bytes_ptr,
                block_group_descriptor_blocks * block_size,
            )
        };

        Self::read_logical_blocks_inner(&mut device, &superblock, block_group_descriptor_block,
                                        descriptor_table_bytes_slice, None)?;

        let root_inode: INode =
            Self::get_inode(&mut device, &superblock, &block_group_descriptor_tables, 2, None)?;
        let root_inode_wrapper: Rc<RefCell<INodeWrapper>> = Rc::new(RefCell::new(INodeWrapper{
            inode: root_inode,
            _inode_num: 2
        }));
        let mut inode_map: BTreeMap<usize, Weak<RefCell<INodeWrapper>>> = BTreeMap::new();
        
        inode_map.insert(2, Rc::downgrade(&root_inode_wrapper));

        Ok(Self { device, superblock, block_group_descriptor_tables,
                  root_inode: root_inode_wrapper, inode_map })
    }

    pub fn get_block_size(&self) -> usize {
        self.superblock.get_block_size()
    }

    pub fn get_inode_size(&mut self) -> u32 {
        self.superblock.s_inode_size as u32
    }

    pub fn find(
        &mut self,
        node: &INodeWrapper,
        name: &[u8],
    ) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        if !node.is_dir() {
            return Err(Ext2Error::NotADirectory);
        }
        let inode_num: Option<u32> = node.get_dir_entries(self, |dir_entry| {
            if &dir_entry.entry.name_characters[0..dir_entry.entry.name_len as usize] == name {
                ControlFlow::Break(dir_entry.entry.inode_num)
            } else {
                ControlFlow::Continue(())
            }
        }, None)?;
        let inode_num: u32 = inode_num.ok_or(Ext2Error::FileNotFound)?;

        let inode: INode = Self::get_inode(&mut self.device, &self.superblock,
                                           &self.block_group_descriptor_tables,
                                           inode_num as usize, None)?;
        let return_value: Rc<RefCell<INodeWrapper>> = Rc::new(RefCell::new(INodeWrapper {
            inode,
            _inode_num: inode_num
        }));

        self.inode_map.insert(inode_num as usize, Rc::downgrade(&return_value));

        Err(Ext2Error::FileNotFound)
    }

    pub fn find_recursive(&mut self, node: Rc<RefCell<INodeWrapper>>, name: &[u8],
                          create_dirs_if_nonexistent: bool,
                          create_file_if_nonexistent: bool) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        let path_split = name.split(|byte| *byte == b'/');
        let path_split_vec = path_split.collect::<Vec<&[u8]>>();
        let mut current_node: Rc<RefCell<INodeWrapper>> = node;

        for (index, file_dir) in path_split_vec.iter().enumerate() {
            let file_dir_str = std::str::from_utf8(file_dir).unwrap();
            let mut current_node_result: Result<Rc<RefCell<INodeWrapper>>, Ext2Error> =
                self.find(&current_node.borrow(), file_dir);

            if current_node_result.is_err() {
                let ext2_error: Ext2Error = current_node_result.unwrap_err();
                let file_not_found: bool = ext2_error == Ext2Error::FileNotFound;

                if file_not_found && index != path_split_vec.len() - 1 && create_dirs_if_nonexistent {
                    let new_node = self.create_dir(&mut *current_node.borrow_mut(), *file_dir)?;

                    current_node = new_node;
                } else if file_not_found && index == path_split_vec.len() - 1 && create_file_if_nonexistent {
                    let new_node = self.create_file(&mut *current_node.borrow_mut(), *file_dir)?;

                    current_node = new_node;
                } else {
                    return Err(ext2_error);
                }
            } else {
                current_node = current_node_result.unwrap();
            }
        }

        Ok(current_node)
    }

    fn acquire_next_available_inode(&mut self, inode_data: INode,
                                    deferred_write_map: &mut DeferredWriteMap) ->
                                                    Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        let block_size: usize = self.superblock.get_block_size();
        let mut found_block_group_index_option: Option<usize> = None;

        for (block_group_index, mut block_group_table)
        in self.block_group_descriptor_tables.iter_mut().enumerate() {
            if block_group_table.bg_free_inodes_count > 1 {
                block_group_table.bg_free_inodes_count -= 1;
                self.superblock.s_free_inodes_count -= 1;
                found_block_group_index_option = Some(block_group_index);
                break;
            }
        }

        if found_block_group_index_option.is_some() {
            let mut block_buffer = vec![0; block_size];
            let found_block_group_index: usize = found_block_group_index_option.unwrap();
            let inode_bitmap_num =
                self.block_group_descriptor_tables[found_block_group_index].bg_inode_bitmap as usize;
            let mut byte_write: [u8; 1] = [0; 1];
            let mut byte_write_pos: usize = 0;

            self.read_logical_block(inode_bitmap_num, &mut block_buffer, Some(deferred_write_map))?;

            let mut found_new_inode: bool = false;
            let new_inode_num_base: usize =
                (self.superblock.s_inodes_per_group as usize) * found_block_group_index;
            let mut new_inode_num: usize = 0;
            let mut inode_byte_offset = 0;

            for (inode_bitmap_byte_index, inode_bitmap_byte)
            in block_buffer.iter().enumerate() {
                for i in 0..8 {
                    let current_relative_inode_num = (inode_bitmap_byte_index * 8) + i;
                    let inode_reserved =
                        current_relative_inode_num < 10 && found_block_group_index == 0;

                    inode_byte_offset = 7 - i;

                    if inode_bitmap_byte & (1 << inode_byte_offset) == 0 && !inode_reserved {
                        new_inode_num += current_relative_inode_num;
                        found_new_inode = true;
                        break;
                    }
                }

                if found_new_inode {
                    break;
                }
            }

            assert!(found_new_inode);
            assert!(!self.inode_map.contains_key(&(new_inode_num + 1)));

            block_buffer[new_inode_num / 8] |= 1 << inode_byte_offset;
            byte_write[0] = block_buffer[new_inode_num / 8];
            byte_write_pos = new_inode_num / 8;

            self.add_write_to_deferred_writes_map(deferred_write_map, inode_bitmap_num, byte_write_pos,
                                                  &byte_write, Some(block_buffer.clone()))?;

            new_inode_num += 1;

            let num_of_inodes_per_block: usize = block_size / size_of::<INode>();
            let inode_block_index: usize =
                (self.block_group_descriptor_tables[found_block_group_index].bg_inode_table as usize) +
                    ((new_inode_num - 1) / num_of_inodes_per_block);
            let inode_block_offset: usize = 
                ((new_inode_num - 1) % num_of_inodes_per_block) * size_of::<INode>();

            self.read_logical_block(inode_block_index, block_buffer.as_mut_slice(),
                                    Some(deferred_write_map))?;

            let inode_bytes = bytemuck::bytes_of(&inode_data);

            block_buffer[inode_block_offset..inode_block_offset + size_of::<INode>()].copy_from_slice(inode_bytes);

            self.add_write_to_deferred_writes_map(deferred_write_map, inode_block_index,
                                                  inode_block_offset, inode_bytes, None)?;

            new_inode_num += new_inode_num_base;

            self.add_block_group_deferred_write(deferred_write_map,
                                                found_block_group_index_option.unwrap())?;
            self.add_super_block_deferred_write(deferred_write_map)?;

            return Ok(Rc::new(RefCell::new(INodeWrapper{
                inode: inode_data,
                _inode_num: new_inode_num as u32
            })));
        }

        Err(Ext2Error::UnavailableINode)
    }

    pub fn add_write_to_deferred_writes_map(&mut self,
                                            deferred_write_map: &mut DeferredWriteMap,
                                            block_num: usize, start_write: usize, write_bytes: &[u8],
                                            optional_block_buffer: Option<Vec<u8>>) -> Result<(), Ext2Error> {
        let block_size: usize = self.superblock.get_block_size();
        let map_no_block = !deferred_write_map.contains_key(&block_num);

        if map_no_block {
            let no_optional_block_buffer: bool = optional_block_buffer.is_none();
            let mut block_buffer: Vec<u8> = if optional_block_buffer.is_some() {
                optional_block_buffer.unwrap()
            } else {
                vec![0; block_size]
            };

            if no_optional_block_buffer {
                self.read_logical_block(block_num, &mut block_buffer, Some(deferred_write_map))?;
            }

            deferred_write_map.insert(block_num, block_buffer);
        }

        deferred_write_map.get_mut(&block_num).unwrap()
            [start_write..start_write+write_bytes.len()].copy_from_slice(write_bytes);

        Ok(())
    }

    pub fn write_back_deferred_writes(&mut self,
                                      mut deferred_writes: DeferredWriteMap) -> Result<(), Ext2Error> {
        self.superblock.s_wtime = get_epoch_time() as u32;
        self.add_super_block_deferred_write(&mut deferred_writes)?;

        for mut deferred_write in deferred_writes {
            self.write_logical_block(deferred_write.0, deferred_write.1.as_mut_slice())?;
        }

        Ok(())
    }
    
    pub fn create_file_with_mode(&mut self, node: &mut INodeWrapper,
                                 name: &[u8], i_mode: u16,
                                 deferred_writes: &mut DeferredWriteMap) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        // what do we need to do when creating a new file?
        // go thru BGD inode bitmaps, find the next unallocated inode number and update it
        // update inode number
        // add a directory entry pointing to our inode thru append_file
        if !node.is_dir() {
            return Err(Ext2Error::InvalidMode);
        }

        let epoch_time: usize = get_epoch_time();

        let new_inode = INode {
            i_mode,
            i_uid: 0x0,
            i_size: 0,
            i_atime: epoch_time as u32,
            i_ctime: epoch_time as u32,
            i_mtime: epoch_time as u32,
            i_dtime: 0,
            i_gid: 0x0,
            i_links_count: 1,
            i_blocks: 0,
            i_flags: 0,
            i_osd1: 0,
            i_block: [0; 15],
            i_generation: 0,
            i_file_acl: 0,
            i_dir_acl: 0,
            i_faddr: 0,
            i_osd2: [0; 12]
        };

        if name.len() > DirectoryEntryConstants::MAX_FILE_NAME_LEN {
            return Err(Ext2Error::TooLongFileName);
        }

        let new_inode_wrapper =
            self.acquire_next_available_inode(new_inode, deferred_writes)?;

        let dir_entry_name_length: u16 =
            std::cmp::min(name.len(), DirectoryEntryConstants::MAX_FILE_NAME_LEN) as u16;
        let mut new_dir_entry_wrapper = DirectoryEntryWrapper {
            entry: DirectoryEntryData {
                inode_num: new_inode_wrapper.borrow()._inode_num,
                rec_len: (DirectoryEntryConstants::MIN_DIRECTORY_ENTRY_SIZE as u16) + dir_entry_name_length,
                name_len: dir_entry_name_length as u8,
                file_type: 0,
                name_characters: [0; 256],
            },
            inode_block_num: 0,
            offset: 0
        };

        let name_string = std::str::from_utf8(name).unwrap();

        new_dir_entry_wrapper.entry.
            name_characters[0..(new_dir_entry_wrapper.entry.name_len as usize)].
            copy_from_slice(name);

        let mut current_inter_block_offset: usize = 0;

        // TODO(Bobby): deal with case where there is enough block space for 
        // TODO(Bobby): dir entry but not padding
        if new_dir_entry_wrapper.entry.rec_len % 4 != 0 {
            new_dir_entry_wrapper.entry.rec_len +=
                4 - (new_dir_entry_wrapper.entry.rec_len % 4);
        }
        let mut found_empty_dir_entry: bool = false;

        let empty_dir_entry_wrapper: Option<DirectoryEntryWrapper> = node.get_dir_entries(self,
                                                                          |dir_entry_wrapper| {
            let mut mutable_entry_for_dir_entry_wrapper = dir_entry_wrapper.entry.clone();
            let mut prior_dir_entry_allocated_size: usize =
                DirectoryEntryConstants::MIN_DIRECTORY_ENTRY_SIZE +
                    (dir_entry_wrapper.entry.name_len as usize);
            let mut dir_entry_padding: usize = 0;

            if (prior_dir_entry_allocated_size % 4) != 0 {
                dir_entry_padding = 4 - (prior_dir_entry_allocated_size % 4);
            }

            current_inter_block_offset += dir_entry_wrapper.entry.rec_len as usize;

            if dir_entry_wrapper.entry.rec_len as usize >=
                prior_dir_entry_allocated_size + dir_entry_padding + new_dir_entry_wrapper.entry.rec_len as usize {
                // resizing prior directory entry to allow our new directory entry
                let prior_dir_entry_new_size =
                    (prior_dir_entry_allocated_size + dir_entry_padding) as u16;

                new_dir_entry_wrapper.inode_block_num = dir_entry_wrapper.inode_block_num;
                new_dir_entry_wrapper.offset =
                    dir_entry_wrapper.offset + (prior_dir_entry_new_size as usize);

                new_dir_entry_wrapper.entry.rec_len =
                    dir_entry_wrapper.entry.rec_len - prior_dir_entry_new_size;
                mutable_entry_for_dir_entry_wrapper.rec_len = prior_dir_entry_new_size;

                found_empty_dir_entry = true;

                ControlFlow::Break(dir_entry_wrapper)
            } else {
                ControlFlow::Continue(())
            }
        }, None)?;

        if empty_dir_entry_wrapper.is_some() {
            let mut dir_entry_wrapper: DirectoryEntryWrapper = empty_dir_entry_wrapper.unwrap();

            dir_entry_wrapper.add_deferred_write(self, node, deferred_writes)?;
            new_dir_entry_wrapper.add_deferred_write(self, node, deferred_writes)?;
        } else {
            // we will need to allocate a new block of dir entries
            let block_size: usize = self.superblock.get_block_size();

            assert_eq!((node.size() as usize) % block_size, 0);
            assert!((block_size - current_inter_block_offset) < new_dir_entry_wrapper.entry.rec_len as usize);

            let mut new_dir_entry: DirectoryEntryData = new_dir_entry_wrapper.entry;
            let mut dir_entry_bytes_with_padding = vec![0; block_size];

            let actual_rec_len: usize = new_dir_entry.rec_len as usize;

            new_dir_entry.rec_len = block_size as u16;
            
            let dir_entry_bytes = bytemuck::bytes_of(&new_dir_entry);
            let remaining_bytes_in_block: usize = block_size - current_inter_block_offset;

            dir_entry_bytes_with_padding[remaining_bytes_in_block..remaining_bytes_in_block + actual_rec_len].copy_from_slice(
                &dir_entry_bytes[0..actual_rec_len]);

            node.append_file_no_writeback(self, dir_entry_bytes_with_padding.as_slice(),
                                          true, deferred_writes)?;
        }

        new_inode_wrapper.borrow_mut().get_deferred_write_inode(self, deferred_writes)?;

        self.inode_map.insert(new_inode_wrapper.borrow()._inode_num as usize,
                              Rc::downgrade(&new_inode_wrapper));

        Ok(new_inode_wrapper)
    }

    // EXT2_S_IROTH and EXT2_S_IXOTH is needed for fuse tests to succeed
    // without sudo escalation
    pub fn create_dir(&mut self, node: &mut INodeWrapper, name: &[u8]) 
        -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        let block_size: usize = self.superblock.get_block_size();
        let mut deferred_writes: DeferredWriteMap = BTreeMap::new();
        let dir_node: Rc<RefCell<INodeWrapper>> =
            self.create_file_with_mode(node, name, EXT2_S_IFDIR | i_mode::EXT2_S_IROTH | i_mode::EXT2_S_IXOTH,
                                       &mut deferred_writes)?;

        let mut cur_dir_entry = DirectoryEntryWrapper {
            entry: DirectoryEntryData {
                inode_num: dir_node.borrow()._inode_num,
                rec_len: (DirectoryEntryConstants::MIN_DIRECTORY_ENTRY_SIZE as u16) + 1,
                name_len: 1,
                file_type: 0,
                name_characters: [0; 256],
            },
            inode_block_num: 0,
            offset: 0
        };

        cur_dir_entry.entry.rec_len += 4 - (cur_dir_entry.entry.rec_len % 4);

        let mut prev_dir_entry = DirectoryEntryWrapper {
            entry: DirectoryEntryData {
                inode_num: node._inode_num,
                rec_len: (DirectoryEntryConstants::MIN_DIRECTORY_ENTRY_SIZE as u16) + 2,
                name_len: 2,
                file_type: 0,
                name_characters: [0; 256],
            },
            inode_block_num: 0,
            offset: cur_dir_entry.entry.rec_len as usize,
        };

        prev_dir_entry.entry.rec_len = (block_size as u16) - cur_dir_entry.entry.rec_len;

        cur_dir_entry.entry.name_characters[0] = b'.';
        prev_dir_entry.entry.name_characters[0] = b'.';
        prev_dir_entry.entry.name_characters[1] = b'.';
        
        let mut dir_entry_bytes = vec![0; block_size];

        cur_dir_entry.copy_to_bytes_slice(&mut dir_entry_bytes, true);
        prev_dir_entry.copy_to_bytes_slice(&mut dir_entry_bytes, true);
        
        dir_node.borrow_mut().append_file_no_writeback(self, &dir_entry_bytes,
                                                       true, &mut deferred_writes)?;

        dir_node.borrow_mut().get_deferred_write_inode(self, &mut deferred_writes)?;

        let writeback_result = self.write_back_deferred_writes(deferred_writes);

        if writeback_result.is_err() {
            Err(writeback_result.unwrap_err())
        } else {
            Ok(dir_node)
        }
    }

    // Creates a file named name (<= 255 characters)
    // EXT2_S_IROTH is needed for fuse tests to succeed
    pub fn create_file(&mut self, node: &mut INodeWrapper,
                       name: &[u8]) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        let mut deferred_writes: DeferredWriteMap = BTreeMap::new();

        let file_node =
            self.create_file_with_mode(node, name, EXT2_S_IFREG | i_mode::EXT2_S_IROTH,
                                       &mut deferred_writes)?;
        let writeback_result = self.write_back_deferred_writes(deferred_writes);

        if writeback_result.is_err() {
            Err(writeback_result.unwrap_err())
        } else {
            Ok(file_node)
        }
    }

    pub fn num_of_block_groups(&self) -> usize {
        let num_of_block_groups_from_blocks: usize =
            ((self.superblock.s_blocks_count as f32) / (self.superblock.s_blocks_per_group as f32)).ceil() as usize;
        let num_of_block_groups_from_inodes: usize =
            ((self.superblock.s_inodes_count as f32) / (self.superblock.s_inodes_per_group as f32)).ceil() as usize;

        assert_eq!(num_of_block_groups_from_blocks, num_of_block_groups_from_inodes);

        num_of_block_groups_from_blocks
    }

    pub fn get_inline_block_capacity(&self) -> usize {
        12
    }

    pub fn get_single_indirect_block_capacity(&self) -> usize {
        self.superblock.get_block_size() / size_of::<u32>()
    }

    pub fn get_double_indirect_block_capacity(&self) -> usize {
        self.get_single_indirect_block_capacity() * self.get_single_indirect_block_capacity()
    }

    pub fn get_triple_indirect_block_capacity(&self) -> usize {
        self.get_single_indirect_block_capacity() * self.get_single_indirect_block_capacity()
    }
}

impl INodeWrapper {
    pub fn is_dir(&self) -> bool {
        (self.inode.i_mode & i_mode::EXT2_S_IFDIR) != 0
    }

    pub fn is_symlink(&self) -> bool {
        (self.inode.i_mode & i_mode::EXT2_S_IFLNK) != 0
    }

    pub fn size(&self) -> u64 {
        // technically, i_dir_acl only has the upper 32 bits
        // for regular files, but it will just be zero for others
        // so it doesn't really matter
        (self.inode.i_size as u64) | ((self.inode.i_dir_acl as u64) << 32)
    }
    
    pub fn update_size<D: BlockDevice>(&mut self, new_size: u64, ext2: &Ext2<D>) {
        self.inode.i_size = ((new_size << 32) >> 32) as u32;
        self.inode.i_dir_acl = (new_size >> 32) as u32;
    }
    
    pub fn get_deferred_write_inode<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>,
                                                    deferred_write_map: &mut DeferredWriteMap) ->
                                                               Result<(), Ext2Error> {
        let inode_block_info: INodeBlockInfo =
            Ext2::get_block_that_has_inode(&mut ext2.device, &ext2.superblock,
                                           &ext2.block_group_descriptor_tables,
                                           self._inode_num as usize);
        let inode_bytes = bytemuck::bytes_of(&self.inode);

        ext2.add_write_to_deferred_writes_map(deferred_write_map, inode_block_info.block_num,
                                              inode_block_info.block_offset, inode_bytes, None)?;

        Ok(())
    }

    pub fn get_block_group_index<D: BlockDevice>(&self, ext2: &Ext2<D>) -> usize {
        (self._inode_num / ext2.superblock.s_inodes_per_group) as usize
    }
    
    pub fn block_allocated_count<D: BlockDevice>(&self, ext2: &Ext2<D>) -> usize {
        (self.inode.i_blocks / (2 << ext2.superblock.s_log_block_size)) as usize
    }
    
    pub fn set_block_allocated_count<D: BlockDevice>(&mut self, ext2: &Ext2<D>, blocks: usize) {
        self.inode.i_blocks = (blocks as u32) * (2 << ext2.superblock.s_log_block_size);
    }

    fn get_word(byte_array: &[u8]) -> u32 {
        u32::from_le_bytes(*byte_array.first_chunk().unwrap())
    }

    pub const TRIPLE_LINK_BLOCK_PTR_INDEX: usize = 14;
    pub const DOUBLE_LINK_BLOCK_PTR_INDEX: usize = 13;
    pub const SINGLE_LINK_BLOCK_PTR_INDEX: usize = 12;

    pub fn get_inode_block_num<D: BlockDevice>(&self, number: usize, ext2: &mut Ext2<D>,
                                               deferred_writes: Option<&DeferredWriteMap>) -> Result<u32, Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let block_inode_list_size: usize = block_size / size_of::<u32>();
        let block_inode_list_size_squared: usize = block_inode_list_size * block_inode_list_size;
        let block_inode_list_size_cubed: usize = block_inode_list_size_squared * block_inode_list_size;

        let mut logical_block_number: u32 = 0;
        let mut block_buffer = vec![0; block_size];

        if number >= (Self::SINGLE_LINK_BLOCK_PTR_INDEX + block_inode_list_size + block_inode_list_size_squared) {
            // hard mode: go through link to list of link of list of links to list of direct
            // block ptrs

            ext2.read_logical_block(self.inode.i_block[Self::TRIPLE_LINK_BLOCK_PTR_INDEX] as usize,
                                         &mut block_buffer, deferred_writes)?;

            let second_level_base_num: usize =
                number - (12 + block_inode_list_size + block_inode_list_size_squared);
            let index: usize =
                (second_level_base_num / block_inode_list_size_squared) * size_of::<u32>();
            let block_second_level_index: u32 =
                Self::get_word(&block_buffer[index..index + 4]);

            ext2.read_logical_block(block_second_level_index as usize,
                                    &mut block_buffer, deferred_writes)?;

            let first_level_base_num: usize = second_level_base_num % block_inode_list_size_squared;
            let block_buffer_second_index: usize =
                (first_level_base_num / block_inode_list_size) * size_of::<u32>();
            let block_first_level_index =
                Self::get_word(
                    &block_buffer[block_buffer_second_index..block_buffer_second_index + 4]);

            ext2.read_logical_block(block_first_level_index as usize, &mut block_buffer,
                                    deferred_writes)?;

            let block_buffer_first_index: usize =
                (first_level_base_num % block_inode_list_size) * size_of::<u32>();

            logical_block_number = Self::get_word(
                &block_buffer[block_buffer_first_index..block_buffer_first_index + 4]);
        } else if number >= Self::SINGLE_LINK_BLOCK_PTR_INDEX + block_inode_list_size {
            // medium: go through link to list of links to list of direct block ptrs
            ext2.read_logical_block(self.inode.i_block[Self::DOUBLE_LINK_BLOCK_PTR_INDEX] as usize,
                                    &mut block_buffer, deferred_writes)?;

            let first_level_base_num: usize = number - (12 + block_inode_list_size);
            let index: usize = (first_level_base_num / block_inode_list_size) * size_of::<u32>();
            let block_first_level_index: usize =
                Self::get_word(&block_buffer[index..index + 4]) as usize;
            let block_final_level_index: usize =
                (first_level_base_num % block_inode_list_size) * size_of::<u32>();

            ext2.read_logical_block(block_first_level_index, &mut block_buffer, deferred_writes)?;

            logical_block_number =
                Self::get_word(&block_buffer[block_final_level_index..block_final_level_index + 4]);
        } else if number >= Self::SINGLE_LINK_BLOCK_PTR_INDEX {
            // fairly easy: go through link to list of direct block ptrs
            ext2.read_logical_block(self.inode.i_block[Self::SINGLE_LINK_BLOCK_PTR_INDEX] as usize,
                                    &mut block_buffer, deferred_writes)?;

            let index: usize = number - 12;
            let offset: usize = index * size_of::<u32>();

            let block_buffer_u32_slice: &[u32] =
                bytemuck::cast_slice::<_, u32>(&mut block_buffer);

            logical_block_number = Self::get_word(&block_buffer[offset..offset + 4]);
        } else {
            // easy: go through direct block ptrs
            logical_block_number = self.inode.i_block[number];
        }

        Ok(logical_block_number)
    }

    pub fn read_block<D: BlockDevice>(
        &self,
        logical_block_start: usize,
        buffer: &mut [u8],
        ext2: &mut Ext2<D>,
        deferred_writes: Option<&DeferredWriteMap>,
    ) -> Result<(), Ext2Error> {
        // TODO: caching
        let block_size = ext2.superblock.get_block_size();

        assert!(buffer.len() % block_size == 0);
        for (buf_segment, block_idx) in buffer
            .chunks_exact_mut(block_size)
            .zip(logical_block_start..)
        {
            let logical_block_num: usize =
                self.get_inode_block_num(block_idx, ext2, deferred_writes)? as usize;
            ext2.read_logical_block(logical_block_num, buf_segment, deferred_writes)?;
        }

        Ok(())
    }

    pub fn read_file<D: BlockDevice>(&self, ext2: &mut Ext2<D>) -> Result<Vec<u8>, Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let mut return_value: Vec<u8> = Vec::new();
        let blocks_to_read: usize = (self.size() as usize).div_ceil(block_size);
        return_value.resize(blocks_to_read * block_size, 0);

        self.read_block(0, return_value.as_mut_slice(), ext2, None)?;

        return_value.resize(self.size() as usize, 0);

        Ok(return_value)
    }

    pub fn get_dir_entries<D: BlockDevice, F, O>(
        &self,
        ext2: &mut Ext2<D>,
        mut callback: F,
        deferred_writes: Option<&DeferredWriteMap>,
    ) -> Result<Option<O>, Ext2Error>
    where
        F: FnMut(DirectoryEntryWrapper) -> ControlFlow<O>,
    {
        // TODO: caching
        let block_size: usize = ext2.superblock.get_block_size();
        let mut buffer = alloc::vec![0; block_size];

        let dir_size: usize = self.size() as usize;
        let dir_blocks: usize = dir_size.div_ceil(block_size);

        let mut i = 0;
        let mut block_idx = 0;

        while block_idx < dir_blocks {
            self.read_block(block_idx, buffer.as_mut_slice(), ext2, deferred_writes)?;

            while i < block_size {
                // TODO: cleanly error on malformed directory entries
                let entry_start = &buffer[i..];
                assert!(entry_start.len() >= size_of::<DirectoryEntryData>());
                let entry_data =
                    unsafe { entry_start.as_ptr().cast::<DirectoryEntryData>().read_unaligned() };
                let name_start = &entry_start[size_of::<DirectoryEntryData>()..];
                let name = &name_start[..entry_data.name_len as usize];

                let entry = DirectoryEntryWrapper {
                    entry: DirectoryEntryData {
                        inode_num: entry_data.inode_num,
                        rec_len: entry_data.rec_len,
                        name_len: entry_data.name_len,
                        file_type: entry_data.file_type,
                        name_characters: [0;256],
                    },
                    inode_block_num: block_idx,
                    offset: 0,
                };

                match callback(entry) {
                    ControlFlow::Continue(()) => (),
                    ControlFlow::Break(res) => return Ok(Some(res)),
                }

                i += entry_data.rec_len as usize;
            }

            let skip_blocks = i / block_size;
            block_idx += skip_blocks;
            i -= block_size * skip_blocks;
            assert_eq!(skip_blocks, 1);
        }

        Ok(None)
    }
    
    pub fn find_new_blocks<D: BlockDevice>(&self, ext2: &mut Ext2<D>,
                                           num_of_blocks: usize,
                                           all_blocks_or_fail: bool,
                                           deferred_writes: &mut DeferredWriteMap)
                                           -> Result<Vec<usize>, Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let num_of_block_groups: usize = ext2.num_of_block_groups();
        let num_of_blocks_per_block_group: usize =
            ext2.superblock.s_blocks_per_group as usize;
        let mut blocks_needed_for_block_bitmap: usize =
            (num_of_blocks_per_block_group / 8) / block_size;

        if (num_of_blocks_per_block_group / 8) % block_size != 0 {
            blocks_needed_for_block_bitmap += 1;
        }

        let mut current_block_group_index: usize = self.get_block_group_index(ext2);
        let mut num_of_blocks_left: usize = num_of_blocks;
        let mut return_value: Vec<usize> = Vec::with_capacity(num_of_blocks);

        while num_of_blocks_left > 0 {
            // TODO: write to block_group_descriptor_tables
            let mut current_block_group: &BGD =
                &ext2.block_group_descriptor_tables[current_block_group_index];

            let mut free_block_count = current_block_group.bg_free_blocks_count as usize;

            while free_block_count == 0 {
                current_block_group_index =
                    (current_block_group_index + 1) % num_of_block_groups;
                current_block_group = &ext2.block_group_descriptor_tables[current_block_group_index];

                free_block_count = current_block_group.bg_free_blocks_count as usize;

                if current_block_group_index == self.get_block_group_index(ext2) {
                    assert_eq!(free_block_count, 0);

                    // we looped around all the block groups, which means there are no more
                    // remaining blocks on this filesystem :(
                    return Err(Ext2Error::NotEnoughDeviceSpace);
                }
            }

            let block_group_base_index: usize =
                num_of_blocks_per_block_group * current_block_group_index;
            let mut blocks_allocated_from_block_group: usize = 0;
            let mut current_block_bitmap_block: usize =
                current_block_group.bg_block_bitmap as usize;
            let last_block_bitmap_block: usize =
                current_block_bitmap_block + blocks_needed_for_block_bitmap;
            let needed_blocks: usize = std::cmp::min(num_of_blocks_left, free_block_count);

            while blocks_allocated_from_block_group < needed_blocks {
                let mut block_buffer = vec![0; block_size];
                let mut block_buffer_dirty: bool = false;
                let mut byte_writes: BTreeMap<usize, u8> = BTreeMap::new();

                ext2.read_logical_block(current_block_bitmap_block, block_buffer.as_mut_slice(),
                                        Some(deferred_writes))?;

                for (index, block_buffer_byte) in block_buffer.iter_mut().enumerate() {
                    let base_block_index: usize = block_group_base_index + (index * 8);

                    for i in 0..8 {
                        if (*block_buffer_byte & (1 << i)) == 0 {
                            let new_block = base_block_index + (i + 1);
                            
                            *block_buffer_byte |= 1 << i;
                            block_buffer_dirty = true;
                            byte_writes.insert(index, *block_buffer_byte);
                            blocks_allocated_from_block_group += 1;

                            return_value.push(new_block);

                            if blocks_allocated_from_block_group >= needed_blocks {
                                break;
                            }
                        }
                    }

                    if blocks_allocated_from_block_group >= needed_blocks {
                        break;
                    }
                }

                if block_buffer_dirty {
                    assert!(blocks_allocated_from_block_group == needed_blocks ||
                            blocks_allocated_from_block_group == free_block_count);

                    ext2.block_group_descriptor_tables[current_block_group_index].
                        bg_free_blocks_count -= blocks_allocated_from_block_group as u16;
                    ext2.superblock.s_free_blocks_count -= blocks_allocated_from_block_group as u32;

                    ext2.add_super_block_deferred_write(deferred_writes)?;
                    ext2.add_block_group_deferred_write(deferred_writes, current_block_group_index)?;

                    for byte_write in byte_writes {
                        ext2.add_write_to_deferred_writes_map(deferred_writes, current_block_bitmap_block,
                                                              byte_write.0, slice::from_ref(&byte_write.1),
                                                              Some(block_buffer.clone()))?;
                    }
                }

                if blocks_allocated_from_block_group < needed_blocks {
                    current_block_bitmap_block += 1;

                    assert!(current_block_bitmap_block < last_block_bitmap_block);
                }
            }

            num_of_blocks_left -= needed_blocks;
        }

        Ok(return_value)
    }

    fn write_indirected_block_to_inode<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>,
                                                       block_num: usize,
                                                       blocks_allocated_within_list: usize,
                                                       num_of_blocks_allocated: &mut usize,
                                                       blocks_newly_allocated: &mut usize,
                                                       new_blocks: &[usize],
                                                       deferred_write_map: &mut DeferredWriteMap) -> Result<(), Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let mut block_buffer = vec![0; block_size];
        let singly_indirect_block_block_limit: usize =
            std::cmp::min(block_size / size_of::<u32>(),
                          new_blocks.len() - *blocks_newly_allocated);

        ext2.read_logical_block(block_num, block_buffer.as_mut_slice(),
                                Some(deferred_write_map))?;

        {
            let mut block_buffer_u32_slice: &mut [u32] =
                bytemuck::cast_slice_mut::<_, u32>(block_buffer.as_mut_slice());

            // handling the contained lists of inode blocks
            for i in blocks_allocated_within_list..singly_indirect_block_block_limit {
                block_buffer_u32_slice[i] = new_blocks[*blocks_newly_allocated] as u32;
                *num_of_blocks_allocated += 1;
                *blocks_newly_allocated += 1;

                if *blocks_newly_allocated == new_blocks.len() {
                    break;
                }
            }

            print!("");
        }

        // TODO(Bobby): change this to a selective write rather than writing the whole buffer
        ext2.add_write_to_deferred_writes_map(deferred_write_map, block_num,
                                              0, &block_buffer,
                                              Some(block_buffer.clone()))?;

        Ok(())
    }

    fn allocate_blocks_for_block_list<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>,
                                                      start_of_list: usize, end_of_list: usize,
                                                      block_list_buffer: &mut [u8],
                                                      new_block_storage_blocks_allocated: &mut usize,
                                                      all_blocks_or_fail: bool, deferred_writes: &mut DeferredWriteMap) -> Result<Vec<usize>, Ext2Error> {
        let mut new_blocks_needed_for_block_list_count: usize = 0;

        ext2.read_logical_block(self.inode.i_block[Self::DOUBLE_LINK_BLOCK_PTR_INDEX] as usize,
                                block_list_buffer, Some(deferred_writes))?;

        let block_list_u32_slice: &[u32] =
            bytemuck::cast_slice::<u8, u32>(block_list_buffer);

        for block_num in start_of_list..end_of_list {
            if block_list_u32_slice[block_num] == UNALLOCATED_BLOCK_SLOT {
                new_blocks_needed_for_block_list_count += 1;
            }
        }

        let result: Result<Vec<usize>, Ext2Error> = self.find_new_blocks(ext2, new_blocks_needed_for_block_list_count,
                                                                         all_blocks_or_fail, deferred_writes);
        if result.is_ok() {
            let result_val = result.unwrap();

            *new_block_storage_blocks_allocated += result_val.len();

            Ok(result_val)
        } else {
            result
        }
    }

    fn write_doublely_indirect_blocks_to_inode<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>,
                                                             double_indirect_block_num: usize,
                                                             num_of_blocks_allocated: &mut usize,
                                                             all_blocks_or_fail: bool,
                                                             blocks_newly_allocated: &mut usize,
                                                             new_block_storage_blocks_allocated: &mut usize,
                                                             new_blocks: &[usize],
                                                             deferred_writes: &mut DeferredWriteMap) -> Result<(), Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let count_of_doubly_linked_blocks: usize = *num_of_blocks_allocated -
            (ext2.get_inline_block_capacity() + ext2.get_single_indirect_block_capacity());

        let starting_doubly_linked_block: usize =
            count_of_doubly_linked_blocks / ext2.get_single_indirect_block_capacity();
        let ending_doubly_linked_block: usize =
            std::cmp::min(ext2.get_single_indirect_block_capacity(),
                          new_blocks.len() -
                              blocks_newly_allocated.div_ceil(ext2.get_single_indirect_block_capacity()));
        let mut block_list_buffer = vec![0; block_size];

        let new_block_list_blocks =
            self.allocate_blocks_for_block_list(ext2, starting_doubly_linked_block,
                                                ending_doubly_linked_block, &mut block_list_buffer,
                                                new_block_storage_blocks_allocated,
                                                all_blocks_or_fail, deferred_writes)?;
        let mut new_block_list_index: usize = 0;

        for block_num in starting_doubly_linked_block..ending_doubly_linked_block {
            let num_of_blocks_allocated_within_list: usize = if block_num == starting_doubly_linked_block {
                (*num_of_blocks_allocated - ext2.get_inline_block_capacity()) % ext2.get_single_indirect_block_capacity()
            } else {
                0
            };

            let mut current_block_slot: u32 =
                bytemuck::cast_slice::<u8, u32>(block_list_buffer.as_mut_slice())[block_num];

            if current_block_slot == UNALLOCATED_BLOCK_SLOT {
                if new_block_list_index >= new_block_list_blocks.len() {
                    return Ok(());
                }

                bytemuck::cast_slice_mut::<u8, u32>(block_list_buffer.as_mut_slice())[block_num] =
                    new_block_list_blocks[new_block_list_index] as u32;
                new_block_list_index += 1;

                ext2.add_write_to_deferred_writes_map(deferred_writes, double_indirect_block_num, block_num*size_of::<u32>(),
                                                      &block_list_buffer[block_num*size_of::<u32>()..(block_num+1)*size_of::<u32>()], None)?;
            }

            current_block_slot =
                bytemuck::cast_slice::<u8, u32>(block_list_buffer.as_mut_slice())[block_num];

            self.write_indirected_block_to_inode(ext2, current_block_slot as usize,
                                                 num_of_blocks_allocated_within_list,
                                                 num_of_blocks_allocated,
                                                 blocks_newly_allocated,
                                                 new_blocks, deferred_writes)?;
        }

        Ok(())
    }

    fn write_triplely_indirect_blocks_to_inode<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>,
                                                               num_of_blocks_allocated: &mut usize,
                                                               all_blocks_or_fail: bool,
                                                               blocks_newly_allocated: &mut usize,
                                                               new_blocks: &[usize],
                                                               new_block_storage_blocks_allocated: &mut usize,
                                                               deferred_writes: &mut DeferredWriteMap) -> Result<(), Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();

        self.allocate_indirect_list_block_if_needed(ext2, Self::TRIPLE_LINK_BLOCK_PTR_INDEX,
                                                    new_block_storage_blocks_allocated,
                                                    all_blocks_or_fail, deferred_writes)?;

        let mut block_list_buffer = vec![0; block_size];

        ext2.read_logical_block(self.inode.i_block[Self::TRIPLE_LINK_BLOCK_PTR_INDEX] as usize,
                                block_list_buffer.as_mut_slice(), Some(deferred_writes))?;

        let starting_triply_directed_list_block: usize =
            (*num_of_blocks_allocated - ext2.get_double_indirect_block_capacity()) / ext2.get_double_indirect_block_capacity();
        let ending_triply_directed_list_block: usize =
            std::cmp::min(ext2.get_triple_indirect_block_capacity(),
                          new_blocks.len() - blocks_newly_allocated.div_ceil(ext2.get_double_indirect_block_capacity()));
        let mut block_list_buffer = vec![0; block_size];

        let new_block_list_blocks: Vec<usize> =
            self.allocate_blocks_for_block_list(ext2, starting_triply_directed_list_block,
                                                ending_triply_directed_list_block,
                                                &mut block_list_buffer,
                                                new_block_storage_blocks_allocated,
                                                all_blocks_or_fail, deferred_writes)?;
        let mut new_block_list_index: usize = 0;
        let triply_indirect_block_list: &mut[u32] = 
            bytemuck::cast_slice_mut::<u8, u32>(block_list_buffer.as_mut_slice());

        for i in starting_triply_directed_list_block..ending_triply_directed_list_block {
            if triply_indirect_block_list[i] == UNALLOCATED_BLOCK_SLOT {
                triply_indirect_block_list[i] = new_block_list_blocks[new_block_list_index] as u32;
                
                new_block_list_index += 1;
            }

            self.write_doublely_indirect_blocks_to_inode(ext2, 
                                                         triply_indirect_block_list[i] as usize,
                                                         num_of_blocks_allocated,
                                                         all_blocks_or_fail, blocks_newly_allocated,
                                                         new_block_storage_blocks_allocated,
                                                         new_blocks, deferred_writes)?;
        }

        Ok(())
    }

    fn allocate_indirect_list_block_if_needed<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>,
                                                              indirect_index: usize,
                                                              new_block_storage_blocks_allocated: &mut usize,
                                                              all_blocks_or_fail: bool,
                                                              deferred_writes: &mut DeferredWriteMap) -> Result<(), Ext2Error>  {
        if self.inode.i_block[indirect_index] == UNALLOCATED_BLOCK_SLOT {
            let new_blocks_allocated: Vec<usize> =
                self.find_new_blocks::<D>(ext2, 1, all_blocks_or_fail,
                                          deferred_writes)?;

            if new_blocks_allocated.is_empty() {
                return Err(NotEnoughDeviceSpace);
            }

            self.inode.i_block[indirect_index] = new_blocks_allocated[0] as u32;

            *new_block_storage_blocks_allocated += 1;
        }
        Ok(())
    }

    fn write_new_blocks_to_inode<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>,
                                                 new_blocks: &[usize],
                                                 new_block_storage_blocks_allocated: &mut usize,
                                                 all_blocks_or_fail: bool,
                                                 deferred_writes: &mut DeferredWriteMap)
                                                 -> Result<usize, Ext2Error> {
        // assumption: references to block zero means unallocated block slot
        // TODO: write inode to disk
        // TODO: write new block num to inode
        // need to handle unallocated blocks containing doubly linked inode blocks
        // and singly linked inode block
        let block_size: usize = ext2.superblock.get_block_size();
        let mut num_of_blocks_allocated: usize = (self.size() as usize).div_ceil(block_size);

        let mut blocks_newly_allocated: usize = 0;

        if num_of_blocks_allocated < ext2.get_inline_block_capacity() &&
            blocks_newly_allocated < new_blocks.len()  {
            for i in num_of_blocks_allocated..12 {
                assert_eq!(self.inode.i_block[i], UNALLOCATED_BLOCK_SLOT);

                self.inode.i_block[i] = new_blocks[blocks_newly_allocated] as u32;
                num_of_blocks_allocated += 1;
                blocks_newly_allocated += 1;

                if blocks_newly_allocated == new_blocks.len() {
                    break;
                }
            }
        }

        let single_block_limit: usize =
            ext2.get_single_indirect_block_capacity() + ext2.get_inline_block_capacity();
        let double_block_limit: usize =
            ext2.get_double_indirect_block_capacity() + single_block_limit;

        if num_of_blocks_allocated < single_block_limit &&
            blocks_newly_allocated < new_blocks.len() {
            let single_indirect_new_blocks_slice: &[usize] = new_blocks;
            let num_of_blocks_allocated_within_list: usize =
                num_of_blocks_allocated - ext2.get_inline_block_capacity();

            self.allocate_indirect_list_block_if_needed(ext2, Self::SINGLE_LINK_BLOCK_PTR_INDEX,
                                                        new_block_storage_blocks_allocated,
                                                        all_blocks_or_fail, deferred_writes)?;

            self.write_indirected_block_to_inode(ext2,
                                                 self.inode.i_block[Self::SINGLE_LINK_BLOCK_PTR_INDEX] as usize,
                                                 num_of_blocks_allocated_within_list,
                                                 &mut num_of_blocks_allocated,
                                                 &mut blocks_newly_allocated,
                                                 single_indirect_new_blocks_slice, deferred_writes)?;
        }

        if blocks_newly_allocated < new_blocks.len() &&
            num_of_blocks_allocated < double_block_limit {
            // find all blocks needed to complete the write
            self.allocate_indirect_list_block_if_needed(ext2, Self::DOUBLE_LINK_BLOCK_PTR_INDEX,
                                                        new_block_storage_blocks_allocated,
                                                        all_blocks_or_fail, deferred_writes)?;

            self.write_doublely_indirect_blocks_to_inode(ext2,
                                                         self.inode.i_block[Self::DOUBLE_LINK_BLOCK_PTR_INDEX] as usize,
                                                         &mut num_of_blocks_allocated,
                                                         all_blocks_or_fail,
                                                         &mut blocks_newly_allocated,
                                                         new_block_storage_blocks_allocated,
                                                         new_blocks, deferred_writes)?;
        }

        if blocks_newly_allocated < new_blocks.len() {
            self.write_triplely_indirect_blocks_to_inode(ext2, &mut num_of_blocks_allocated,
                                                         all_blocks_or_fail, &mut blocks_newly_allocated,
                                                         new_blocks, new_block_storage_blocks_allocated,
                                                         deferred_writes)?;
        }

        Ok(blocks_newly_allocated)
    }

    fn append_file_no_writeback<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>, new_data: &[u8],
                                all_bytes_or_fail: bool,
                                deferred_writes: &mut DeferredWriteMap) -> Result<usize, Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let allocated_block_count: usize = (self.size() as usize).div_ceil(block_size);
        let base_allocated_block: Option<usize> =
            if allocated_block_count == 0 { None } else {
                Some(self.get_inode_block_num(allocated_block_count - 1, ext2,
                                              Some(deferred_writes))? as usize)
            };

        let base_allocated_block_offset: usize = (self.size() as usize) % block_size;
        let mut new_blocks_allocated: Vec<usize> = Vec::new();
        let mut new_block_storage_blocks_allocated: usize = 0;
        let mut bytes_written: usize = 0;

        if base_allocated_block_offset > 0 {
            new_blocks_allocated.push(base_allocated_block.unwrap());
        }

        let image_file_size_in_blocks: usize = new_data.len().div_ceil(block_size);

        // if we have no space left in our base block or we have insufficient enough space to only
        // use the blocks we have, then we go looking for more blocks
        if base_allocated_block_offset == 0 ||
           (block_size - base_allocated_block_offset) < new_data.len() {
            let new_blocks_allocated_result =
                self.find_new_blocks::<D>(ext2, image_file_size_in_blocks, all_bytes_or_fail,
                                          deferred_writes);

            let old_new_blocks_allocated_size: usize = new_blocks_allocated.len();

            new_blocks_allocated.append(&mut new_blocks_allocated_result?);

            let new_blocks_slice: &[usize] =
                &new_blocks_allocated[old_new_blocks_allocated_size..];
            
            self.write_new_blocks_to_inode(ext2, new_blocks_slice,
                                           &mut new_block_storage_blocks_allocated,
                                           true, deferred_writes)?;
        }

        let new_data_block_allocated_num: usize =
            allocated_block_count + new_blocks_allocated.len();

        for new_block in new_blocks_allocated {
            let write_base = if base_allocated_block.is_some() && new_block == base_allocated_block.unwrap() {
                base_allocated_block_offset
            } else {
                0
            };
            let write_size = std::cmp::min(if base_allocated_block.is_some() && new_block == base_allocated_block.unwrap() {
                block_size - base_allocated_block_offset
            } else {
                block_size
            }, new_data.len() - bytes_written);

            let current_byte_slice: &[u8] = &new_data[bytes_written..bytes_written+write_size];

            bytes_written += write_size;

            ext2.add_write_to_deferred_writes_map(deferred_writes, new_block, write_base,
                                                  current_byte_slice, None)?;
        }

        self.update_size(self.size() + (bytes_written as u64), ext2);
        self.set_block_allocated_count(ext2,
                                       new_data_block_allocated_num + new_block_storage_blocks_allocated);
        self.get_deferred_write_inode(ext2, deferred_writes)?;

        Ok(bytes_written)
    }

    // append to file, with the new file size being the existing file size + size of new_data
    pub fn append_file<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>, new_data: &[u8],
                                       all_bytes_or_fail: bool) -> Result<usize, Ext2Error> {
        let mut deferred_writes: DeferredWriteMap = BTreeMap::new();
        let bytes_written: usize =
            self.append_file_no_writeback(ext2, new_data, all_bytes_or_fail, &mut deferred_writes)?;

        ext2.write_back_deferred_writes(deferred_writes)?;

        Ok(bytes_written)
    }

    pub fn truncate_file<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>, num_bytes: u64) -> Result<u64, Ext2Error> {
        // TODO: 
        let mut deferred_writes = BTreeMap::new();
        let block_size: usize = ext2.superblock.get_block_size();
        let block_info: INodeBlockInfo =
            Ext2::get_block_that_has_inode(&mut ext2.device, &ext2.superblock, &ext2.block_group_descriptor_tables, self._inode_num as usize);
        if self.size() > num_bytes {
            return Err(Ext2Error::FileSizeMismatch);
        } else if (self.size() - num_bytes) / block_size as u64 == self.size() / block_size as u64 {
            // easy case
            self.update_size(self.size() - num_bytes, ext2);
            self.get_deferred_write_inode(ext2, &mut deferred_writes)?;
        } else {
            let num_blocks_remaining =
                (self.size() as usize - num_bytes as usize).div_ceil(block_size);
            let num_blocks_removed =
                (self.size() as usize).div_ceil(block_size) - num_blocks_remaining;
            let num_bytes_remaining_removed = num_bytes as usize - num_blocks_removed * block_size;
            
            // unallocate num_blocks_removed blocks
            for i in (num_blocks_remaining..(self.size() as usize).div_ceil(block_size)).rev() {
                let logical_block_num = self.get_inode_block_num(i, ext2, Some(&deferred_writes))?;
                // update block bitmap
                let block_group_num = logical_block_num / ext2.superblock.s_blocks_per_group;
                let index = logical_block_num % ext2.superblock.s_blocks_per_group;

                let mut block_buffer = vec![0; block_size];

                ext2.read_logical_block(
                    ext2.block_group_descriptor_tables[block_group_num as usize].bg_block_bitmap as usize,
                    block_buffer.as_mut_slice(), Some(&deferred_writes))?;
                let mut block_buffer_byte = block_buffer[index as usize / 8];
                block_buffer_byte &= 0b11111111 - (1 << (index % 8));
                ext2.add_write_to_deferred_writes_map(&mut deferred_writes, ext2.block_group_descriptor_tables[block_group_num as usize].bg_block_bitmap as usize, index as usize, slice::from_ref(&block_buffer_byte), Some(block_buffer))?;

                // increment free blocks count
                ext2.block_group_descriptor_tables[block_group_num as usize].bg_free_blocks_count += 1;
                ext2.superblock.s_free_blocks_count += 1;

                ext2.add_block_group_deferred_write(&mut deferred_writes, block_group_num as usize)?;
            }
            ext2.add_super_block_deferred_write(&mut deferred_writes)?;

            self.update_size(self.size() - num_bytes_remaining_removed as u64, ext2);
            self.get_deferred_write_inode(ext2, &mut deferred_writes)?;
        }
        ext2.write_back_deferred_writes(deferred_writes)?;
        Ok(num_bytes)
    }

    // overwrite over file, with the new file size being the size of new_data
    pub fn overwrite_file<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>, new_data: &[u8],
                                          all_bytes_or_fail: bool) -> Result<usize, Ext2Error> {
        // TODO(Sasha): Handle partial writes, properly report bytes written
        let block_size: usize = ext2.superblock.get_block_size();
        let allocated_block_count = (self.size() as usize).div_ceil(block_size);
        let mut deferred_writes: DeferredWriteMap = BTreeMap::new();
        let mut bytes_written: u64 = 0;
        if allocated_block_count == new_data.len().div_ceil(block_size) {
            // easy case
            for i in 0..allocated_block_count {
                let cur_block = self.get_inode_block_num(i, ext2, Some(&deferred_writes))? as usize;
                let current_byte_slice: &[u8] = &new_data[i*block_size..std::cmp::min((i+1)*block_size, new_data.len())];
                ext2.add_write_to_deferred_writes_map(&mut deferred_writes, cur_block, 0,
                    current_byte_slice, None)?;
                bytes_written += (std::cmp::min((i+1)*block_size, new_data.len()) - i*block_size) as u64;
            }
            self.update_size(bytes_written, ext2);
            self.get_deferred_write_inode(ext2, &mut deferred_writes)?;
            ext2.write_back_deferred_writes(deferred_writes)?;
        } else if allocated_block_count < new_data.len().div_ceil(block_size) {
            // allocate more
            for i in 0..allocated_block_count {
                let cur_block = self.get_inode_block_num(i, ext2, Some(&deferred_writes))? as usize;
                let current_byte_slice: &[u8] = &new_data[i*block_size..(i+1)*block_size];
                ext2.add_write_to_deferred_writes_map(&mut deferred_writes, cur_block, 0,
                    current_byte_slice, None)?;
                bytes_written += ((i+1)*block_size - i*block_size) as u64;
            }
            self.update_size(bytes_written, ext2);
            self.get_deferred_write_inode(ext2, &mut deferred_writes)?;
            ext2.write_back_deferred_writes(deferred_writes)?;
            let new_slice: &[u8] = &new_data[allocated_block_count*block_size..new_data.len()];
            bytes_written += self.append_file(ext2, new_slice, all_bytes_or_fail)? as u64;
        } else {
            self.truncate_file(ext2, new_data.len() as u64 - self.size() as u64);
            for i in 0..allocated_block_count {
                let cur_block = self.get_inode_block_num(i, ext2, Some(&deferred_writes))? as usize;
                let current_byte_slice: &[u8] = &new_data[i*block_size..std::cmp::min((i+1)*block_size, new_data.len())];
                ext2.add_write_to_deferred_writes_map(&mut deferred_writes, cur_block, 0,
                    current_byte_slice, None)?;
                bytes_written += (std::cmp::min((i+1)*block_size, new_data.len()) - i*block_size) as u64;
            }
            self.update_size(bytes_written, ext2);
            self.get_deferred_write_inode(ext2, &mut deferred_writes)?;
            ext2.write_back_deferred_writes(deferred_writes)?;
        }
        Ok(new_data.len())
    }

    pub fn delete_file<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>) -> Result<usize, Ext2Error> {
        let block_size = ext2.superblock.get_block_size();
        let size = self.size() as usize;
        self.truncate_file(ext2, self.size());
        let actual_inode_num = self._inode_num - 1; // inodes are 1 indexed
        let block_group_num = actual_inode_num as usize / ext2.superblock.s_blocks_per_group as usize;
        let index = actual_inode_num as usize % ext2.superblock.s_blocks_per_group as usize;
        let mut block_buffer = vec![0; block_size];

        let mut deferred_writes: DeferredWriteMap = BTreeMap::new();

        ext2.read_logical_block(ext2.block_group_descriptor_tables[block_group_num as usize].bg_inode_bitmap as usize,
                                block_buffer.as_mut_slice(), Some(&deferred_writes))?;
        let mut block_buffer_byte = block_buffer[index as usize / 8];
        block_buffer_byte &= 0b11111111 - (1 << (index % 8));
        ext2.add_write_to_deferred_writes_map(&mut deferred_writes, ext2.block_group_descriptor_tables[block_group_num as usize].bg_inode_bitmap as usize, index as usize, slice::from_ref(&block_buffer_byte), Some(block_buffer))?;

        ext2.block_group_descriptor_tables[block_group_num].bg_free_inodes_count -= 1;
        ext2.superblock.s_free_inodes_count += 1;
        //TODO: if this is the last inode in this block, should we deallocate the block?
        ext2.add_block_group_deferred_write(&mut deferred_writes, block_group_num as usize)?;
        ext2.add_super_block_deferred_write(&mut deferred_writes)?;
        //TODO: probably want to delete myself
        Ok(size)
    }
}
