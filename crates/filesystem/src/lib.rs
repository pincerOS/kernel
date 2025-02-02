#![no_std]

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
use alloc::rc::{Rc, Weak};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs::DirEntry;
use std::prelude::v1::{String, Vec};
use std::ptr::{read, write};
use std::time::{SystemTime, UNIX_EPOCH};
use bytemuck::bytes_of;
use crate::i_mode::EXT2_S_IFREG;

#[cfg(test)]
mod tests;

#[cfg(feature = "std")]
pub mod linux;

pub const SECTOR_SIZE: usize = 512;
pub const BLOCK_SIZE: usize = 1024;

#[derive(Debug)]
pub enum BlockDeviceError {
    Unknown,
}

#[derive(Debug)]
pub enum Ext2Error {
    BlockDeviceError(BlockDeviceError),
    UnavailableINode,
    TooLongFileName,
    InvalidMode
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
        sectors: usize,
        buffer: &mut [u8],
    ) -> Result<(), BlockDeviceError> {
        let mut tmp_buf: [u8; 512] = [0; 512];
        for i in 0..sectors {
            let cur_sector = start_index + (i as u64);
            self.read_sector(cur_sector, &mut tmp_buf)?;
            for j in 0..SECTOR_SIZE {
                buffer[(i*SECTOR_SIZE)+j] = tmp_buf[j];
            }
        }
        Ok(())
    }
    
    fn write_sectors(
        &mut self,
        start_index: u64,
        sectors: usize,
        buffer: &mut [u8],
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

type DeferredWriteMap = BTreeMap<usize, [u8; BLOCK_SIZE]>;

mod DirectoryEntryConstants {
    pub const MAX_FILE_NAME_LEN: usize = 32;
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DirectoryEntry {
    // assumption: name length <= 2^16
    // all part of the spec
    inode_number: u32,
    entry_size: u16,
    name_length: u16,

    name_characters: [u8; DirectoryEntryConstants::MAX_FILE_NAME_LEN]
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
	s_uuid: [u8;16],
    s_volume_name: [u8;16],
    s_last_mounted: [u8;64],
    s_algo_bitmap: u32,
	s_prealloc_blocks: u8,
	s_prealloc_dir_blocks: u8,
    unused_alignment_1: [u8;2],
    s_journal_uuid: [u8;16],
	s_journal_inum: u32,
	s_journal_dev: u32,
	s_last_orphan: u32,
    s_hash_seed: [u32;4],
    s_def_hash_version: u8,
    unused_alignment_2: [u8;3],
    s_default_mount_options: u32,
    s_first_meta_bg: u32,
    // for some reason (padding?) unused_alignment_4: [u8; 760] causes
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
}

pub mod s_state {
    const EXT2_VALID_FS: u16 = 1;
    const EXT2_ERROR_FS: u16 = 2;
}

pub mod s_errors {
    const EXT2_ERRORS_CONTINUE: u16 = 1;
    const EXT2_ERRORS_RO: u16 = 2;
    const EXT2_ERRORS_PANIC: u16 = 3;
}

pub mod s_creator_os {
    const EXT2_OS_LINUX: u32 = 0;
    const EXT2_OS_HURD: u32 = 1;
    const EXT2_OS_MASIX: u32 = 2;
    const EXT2_OS_FREEBSD: u32 = 3;
    const EXT2_OS_LITES: u32 = 4;
}

pub mod s_rev_level {
    const EXT2_GOOD_OLD_REV: u32 = 0;
    const EXT2_DYNAMIC_REV: u32 = 1;
}

pub mod s_feature_compat {
    const EXT2_FEATURE_COMPAT_DIR_PREALLOC: u32 = 0x0001;
    const EXT2_FEATURE_COMPAT_IMAGIC_INODES: u32 = 0x0002;
    const EXT3_FEATURE_COMPAT_HAS_JOURNAL: u32 = 0x0004;
    const EXT2_FEATURE_COMPAT_EXT_ATTR: u32 = 0x0008;
    const EXT2_FEATURE_COMPAT_RESIZE_INO: u32 = 0x0010;
    const EXT2_FEATURE_COMPAT_DIR_INDEX: u32 = 0x0020;
}

pub mod s_feature_incompat {
    const EXT2_FEATURE_INCOMPAT_COMPRESSION: u32 = 0x0001;
    const EXT2_FEATURE_INCOMPAT_FILETYPE: u32 = 0x0002;
    const EXT3_FEATURE_INCOMPAT_RECOVER: u32 = 0x0004;
    const EXT3_FEATURE_INCOMPAT_JOURNAL_DEV: u32 = 0x0008;
    const EXT2_FEATURE_INCOMPAT_META_BG: u32 = 0x0010;
}

pub mod s_feature_ro_compat {
    const EXT2_FEATURE_RO_COMPAT_SPARSE_SUPER: u32 = 0x0001;
    const EXT2_FEATURE_RO_COMPAT_LARGE_FILE: u32 = 0x0002;
    const EXT2_FEATURE_RO_COMPAT_BTREE_DIR: u32 = 0x0004;
}

pub mod s_algo_bitmap {
    const EXT2_LZV1_ALG: u32 = 0x0001;
    const EXT2_LZRW3A_ALG: u32 = 0x0002;
    const EXT2_GZIP_ALG: u32 = 0x0004;
    const EXT2_BZIP2_ALG: u32 = 0x0008;
    const EXT2_LZO_ALG: u32 = 0x0010;
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
    bg_reserved: [u8;12], 
}

const _: () = assert!(size_of::<BGD>() == 32);

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
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
    i_block: [u32;15], // 12 direct, single, double, triple
    i_generation: u32,
    i_file_acl: u32,
    i_dir_acl: u32,
    i_faddr: u32,
    i_osd2: [u8;12],
}

struct INodeBlockInfo {
    block_num: usize,
    block_offset: usize,
}

pub mod reserved_inodes {
    const EXT2_BAD_INO: u32 = 1;
    const EXT2_ROOT_INO: u32 = 2;
    const EXT2_ACL_IDX_INO: u32 = 3;
    const EXT2_ACL_DATA_INO: u32 = 4;
    const EXT2_BOOT_LOADER_INO: u32 = 5;
    const EXT2_UNDEL_DIR_INO: u32 = 6;
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

pub struct INodeWrapper {
    inode: INode,
    inode_num: u32
}

// TODO(Bobby): replace this with how we get time without std
fn get_epoch_time() -> usize {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as usize
}

impl<D> Ext2<D>
where
    D: BlockDevice,
{

    fn read_logical_block(device: &mut D, logical_block_start: usize, logical_block_length: usize,
                          buffer: &mut [u8]) -> Result<(), Ext2Error> {
        assert!(logical_block_length > 0);

        let start_sector_numerator: usize = logical_block_start * BLOCK_SIZE;
        let start_sector: usize = start_sector_numerator / SECTOR_SIZE;
        let sectors: usize = (logical_block_length * BLOCK_SIZE) / SECTOR_SIZE;

        let read_result: Result<(), BlockDeviceError> =
            device.read_sectors(start_sector as u64, sectors, buffer);

        if read_result.is_ok() {
            Ok(())
        } else {
            Err(Ext2Error::BlockDeviceError(read_result.unwrap_err()))
        }
    }

    fn write_logical_block(device: &mut D, logical_block_start: usize, logical_block_length: usize,
                           buffer: &mut [u8]) -> Result<(), Ext2Error> {
        assert!(logical_block_length > 0);

        let start_sector_numerator: usize = logical_block_start * BLOCK_SIZE;
        let start_sector: usize = start_sector_numerator / SECTOR_SIZE;
        let sectors: usize = (logical_block_length * BLOCK_SIZE) / SECTOR_SIZE;

        let write_result: Result<(), BlockDeviceError> =
            device.write_sectors(start_sector as u64, sectors, buffer);

        if write_result.is_ok() {
            Ok(())
        } else {
            Err(Ext2Error::BlockDeviceError(write_result.unwrap_err()))
        }
    }

    pub fn write_logical_block_self(&mut self, logical_block_start: usize,
                                    logical_block_length: usize, buffer: &mut [u8]) -> Result<(), Ext2Error> {
        Self::write_logical_block(&mut self.device, logical_block_start, logical_block_length,
                                  buffer)
    }

    pub fn read_logical_block_self(&mut self, logical_block_start: usize, logical_block_length: usize,
                                   buffer: &mut [u8]) -> Result<(), Ext2Error> {
        Self::read_logical_block(&mut self.device, logical_block_start, logical_block_length, 
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
            ((inode_table_index * inode_size) / BLOCK_SIZE) + inode_table_block;
        let inode_table_interblock_offset: usize = (inode_table_index * inode_size) % BLOCK_SIZE;

        INodeBlockInfo{
            block_num: inode_table_block_with_offset,
            block_offset: inode_table_interblock_offset
        }
    }

    fn get_inode(device: &mut D, superblock: &Superblock, block_group_descriptor_tables: &Vec<BGD>,
                 inode_num: usize) -> INode {
        let inode_block_info: INodeBlockInfo =
            Ext2::get_block_that_has_inode(device, superblock, block_group_descriptor_tables, inode_num);
        let mut block_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];

        Self::read_logical_block(device, inode_block_info.block_num, 1,
                                 &mut block_buffer);

        let mut inode_data: [u8; size_of::<INode>()] = [0x00; size_of::<INode>()];

        inode_data.copy_from_slice(
            &block_buffer[inode_block_info.block_offset..inode_block_info.block_offset + size_of::<INode>()]);

        let inode: INode =
            unsafe {std::mem::transmute::<[u8;size_of::<INode>()], INode>(inode_data)};

        inode
    }

    pub fn get_root_inode_wrapper(&mut self) -> Rc<RefCell<INodeWrapper>> {
        self.root_inode.clone()
    }

    pub fn add_block_group_deferred_write(&mut self,
                                          deferred_write_map: &mut DeferredWriteMap,
                                          block_group_num: usize) -> Result<(), Ext2Error> {
        let block_group_descriptor_block: usize =
            if BLOCK_SIZE == 1024 {2} else {1} + ((block_group_num * size_of::<BGD>()) / BLOCK_SIZE);
        let block_group_descriptor_offset: usize =
            (block_group_num * size_of::<BGD>()) % BLOCK_SIZE;

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

    pub fn new(mut device: D) -> Self {
        let mut buffer: [u8; 1024] = [0; 1024];
        // todo: error-handling device.read_sectors
        device.read_sectors(2, 2, &mut buffer);
        let superblock: Superblock = unsafe { std::mem::transmute::<[u8; 1024], Superblock>(buffer) };

        let mut block_group_descriptor_tables: Vec<BGD> = Vec::new();

        let block_group_descriptor_block: usize = if BLOCK_SIZE == 1024 {2} else {1};
        let block_group_descriptor_length: usize =
            1 + (((superblock.get_num_of_block_groups() as usize) * size_of::<BGD>()) / BLOCK_SIZE);

        let num_of_descriptor_tables: usize =
            block_group_descriptor_length * (BLOCK_SIZE / size_of::<BGD>());

        block_group_descriptor_tables.resize(num_of_descriptor_tables,
                                             BGD{ bg_block_bitmap: 0, bg_inode_bitmap: 0, 
                                                        bg_inode_table: 0, bg_free_blocks_count: 0, 
                                                        bg_free_inodes_count: 0, bg_used_dirs_count: 0, 
                                                        bg_pad: 0,  bg_reserved: [0;12] });

        let descriptor_table_bytes_ptr: *mut u8 =
            block_group_descriptor_tables.as_mut_ptr() as *mut u8;
        let descriptor_table_bytes_slice: &mut[u8] = unsafe{
            std::slice::from_raw_parts_mut(descriptor_table_bytes_ptr,
                                           block_group_descriptor_length * BLOCK_SIZE)
        };

        Self::read_logical_block(&mut device, block_group_descriptor_block,
                                 block_group_descriptor_length, descriptor_table_bytes_slice);

        let root_inode: INode =
            Self::get_inode(&mut device, &superblock, &block_group_descriptor_tables, 2);
        let root_inode_wrapper: Rc<RefCell<INodeWrapper>> = Rc::new(RefCell::new(INodeWrapper{
            inode: root_inode,
            inode_num: 2
        }));
        let mut inode_map: BTreeMap<usize, Weak<RefCell<INodeWrapper>>> = BTreeMap::new();
        
        inode_map.insert(2, Rc::downgrade(&root_inode_wrapper));

        Self { device, superblock, block_group_descriptor_tables, 
               root_inode: root_inode_wrapper, inode_map }
    }

    pub fn get_block_size(&mut self) -> u32 {
        1024 << self.superblock.s_log_block_size
    }

    pub fn get_inode_size(&mut self) -> u32 {
        self.superblock.s_inode_size as u32
    }

    pub fn find(&mut self, node: &INodeWrapper, name: &[u8]) -> Option<Rc<RefCell<INodeWrapper>>> {
        let dir_entries: Vec<DirectoryEntry> = node.get_dir_entries(self);

        if node.is_dir() {
            for dir_entry in dir_entries {
                if dir_entry.name_length == name.len() as u16 &&
                   &dir_entry.name_characters[0..name.len()] == name {
                    // TODO: find out how to do operator overloading in rust and convert this into
                    // TODO: a mut self method
                    if self.inode_map.contains_key(&(dir_entry.inode_number as usize)) {
                        let inode_strong_ref = 
                            self.inode_map.get(&(dir_entry.inode_number as usize)).unwrap().upgrade();
                        
                        if inode_strong_ref.is_some() {
                            return Some(inode_strong_ref.unwrap());
                        }
                    }
                    
                    let inode: INode = Self::get_inode(&mut self.device, &self.superblock,
                                                       &self.block_group_descriptor_tables,
                                                       dir_entry.inode_number as usize);
                    let return_value: Rc<RefCell<INodeWrapper>> = Rc::new(RefCell::new(INodeWrapper {
                        inode,
                        inode_num: dir_entry.inode_number
                    }));

                    self.inode_map.insert(dir_entry.inode_number as usize, 
                                          Rc::downgrade(&return_value));
                    
                    return Some(return_value);
                }
            }
        }

        None
    }

    fn acquire_next_available_inode(&mut self, inode_data: INode,
                                    deferred_write_map: &mut DeferredWriteMap) ->
                                                    Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
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
            let mut block_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
            let found_block_group_index: usize = found_block_group_index_option.unwrap();
            let inode_bitmap_num =
                self.block_group_descriptor_tables[found_block_group_index].bg_inode_bitmap as usize;
            let mut byte_write: [u8; 1] = [0; 1];
            let mut byte_write_pos: usize = 0;

            self.read_logical_block_self(inode_bitmap_num,1, &mut block_buffer)?;

            let mut found_new_inode: bool = false;
            let new_inode_num_base: usize =
                (self.superblock.s_inodes_per_group as usize) * found_block_group_index;
            let mut new_inode_num: usize = 0;

            for (inode_bitmap_byte_index, inode_bitmap_byte)
            in block_buffer.iter().enumerate() {
                for i in 0..8 {
                    if inode_bitmap_byte & (1 << (7 - i)) == 0 {
                        new_inode_num += (inode_bitmap_byte_index * 8) + i;
                        found_new_inode = true;
                        break;
                    }
                }

                if found_new_inode {
                    break;
                }
            }

            assert!(found_new_inode);
            block_buffer[new_inode_num / 8] |= 1 << (new_inode_num % 8);
            byte_write[0] = block_buffer[new_inode_num / 8];
            byte_write_pos = new_inode_num / 8;

            self.add_write_to_deferred_writes_map(deferred_write_map, inode_bitmap_num, byte_write_pos,
                                                  &byte_write, Some(block_buffer))?;

            new_inode_num += 1;

            let num_of_inodes_per_block: usize = BLOCK_SIZE / size_of::<INode>();
            let inode_block_index: usize =
                (self.block_group_descriptor_tables[found_block_group_index].bg_inode_table as usize) +
                    ((new_inode_num - 1) / num_of_inodes_per_block);
            let inode_block_offset: usize = (new_inode_num - 1) % num_of_inodes_per_block;

            self.read_logical_block_self(inode_block_index, 1,
                                         &mut block_buffer)?;

            let inode_bytes = bytemuck::bytes_of(&inode_data);

            block_buffer[inode_block_offset..inode_block_offset + size_of::<INode>()].copy_from_slice(inode_bytes);

            self.add_write_to_deferred_writes_map(deferred_write_map, inode_bitmap_num,
                                                  inode_block_offset, inode_bytes, None)?;

            new_inode_num += new_inode_num_base;

            self.add_block_group_deferred_write(deferred_write_map,
                                                found_block_group_index_option.unwrap())?;
            self.add_super_block_deferred_write(deferred_write_map)?;

            return Ok(Rc::new(RefCell::new(INodeWrapper{
                inode: inode_data,
                inode_num: new_inode_num as u32
            })));
        }

        Err(Ext2Error::UnavailableINode)
    }

    pub fn add_write_to_deferred_writes_map(&mut self,
                                            deferred_write_map: &mut DeferredWriteMap,
                                            block_num: usize, start_write: usize, write_bytes: &[u8],
                                            optional_block_buffer: Option<[u8; BLOCK_SIZE]>) -> Result<(), Ext2Error> {
        if !deferred_write_map.contains_key(&block_num) {
            let mut block_buffer: [u8; BLOCK_SIZE] = if optional_block_buffer.is_some() {
                optional_block_buffer.unwrap()
            } else {
                [0; BLOCK_SIZE]
            };

            if optional_block_buffer.is_none() {
                self.read_logical_block_self(block_num, 1, &mut block_buffer)?;
            }

            deferred_write_map.insert(block_num, block_buffer);
        }

        deferred_write_map.get_mut(&block_num).unwrap()
            [start_write..start_write+write_bytes.len()].copy_from_slice(write_bytes);

        Ok(())
    }

    pub fn write_back_deferred_writes(&mut self,
                                      mut deferred_writes: DeferredWriteMap) -> Result<(), Ext2Error> {
        let start = SystemTime::now();
        let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap();

        self.superblock.s_wtime = since_the_epoch.as_secs() as u32;
        self.add_super_block_deferred_write(&mut deferred_writes)?;

        for mut deferred_write in deferred_writes {
            self.write_logical_block_self(deferred_write.0, 1,
                                          &mut deferred_write.1)?;
        }

        Ok(())
    }

    // Creates a file named name (<= 255 characters),
    // returns None if out of disk space
    pub fn create_file(&mut self, node: &mut INodeWrapper,
                       name: &[u8]) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        // what do we need to do when creating a new file?
        // go thru BGD inode bitmaps, find the next unallocated inode number and update it
        // update inode number
        // add a directory entry pointing to our inode thru append_file
        if !node.is_dir() {
            return Err(Ext2Error::InvalidMode);
        }

        let epoch_time: usize = get_epoch_time();

        let new_inode = INode {
            i_mode: EXT2_S_IFREG,
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

        let mut deferred_writes: DeferredWriteMap = BTreeMap::new();
        let new_inode_wrapper =
            self.acquire_next_available_inode(new_inode, &mut deferred_writes)?;

        let dir_entry_name_length: u16 =
            std::cmp::min(name.len(), DirectoryEntryConstants::MAX_FILE_NAME_LEN) as u16;
        let mut new_directory_entry = DirectoryEntry {
            inode_number: new_inode_wrapper.borrow().inode_num,
            entry_size: 8 + dir_entry_name_length,
            name_length: dir_entry_name_length,
            name_characters: [0; DirectoryEntryConstants::MAX_FILE_NAME_LEN],
        };

        new_directory_entry.name_characters[0..(new_directory_entry.name_length as usize)].copy_from_slice(name);

        let dir_entry_bytes = bytemuck::bytes_of(&new_directory_entry);
        let current_inter_block_offset: usize = node.size() as usize % BLOCK_SIZE;
        let remaining_bytes_in_block: usize = BLOCK_SIZE - current_inter_block_offset;
        let mut dir_entry_bytes_with_padding: Vec<u8> = Vec::new();
        let mut dir_entry_padding: usize = 0;

        dir_entry_bytes_with_padding.resize(new_directory_entry.entry_size as usize, 0);

        // dir entries need to be at 4-byte alignment
        if (current_inter_block_offset % 4) > 0 {
            let four_byte_padding: usize = 4 - (current_inter_block_offset % 4);
            dir_entry_padding += four_byte_padding;

            dir_entry_bytes_with_padding.resize(dir_entry_bytes_with_padding.len() + four_byte_padding,
                                                0);
        }

        // and they need to not cross block boundaries
        if dir_entry_bytes_with_padding.len() >= remaining_bytes_in_block {
            dir_entry_padding += remaining_bytes_in_block;

            dir_entry_bytes_with_padding.resize(dir_entry_bytes_with_padding.len() + remaining_bytes_in_block, 0);
        }
        
        let actual_entry_size: usize = new_directory_entry.entry_size as usize;
        let slice_end: usize = dir_entry_padding + actual_entry_size;

        dir_entry_bytes_with_padding[dir_entry_padding..slice_end].copy_from_slice(
            &dir_entry_bytes[0..actual_entry_size]);

        node.append_file_no_writeback(self, dir_entry_bytes_with_padding.as_slice(),
                                      true, &mut deferred_writes)?;

        self.write_back_deferred_writes(deferred_writes)?;
        
        self.inode_map.insert(new_inode_wrapper.borrow().inode_num as usize,
                              Rc::downgrade(&new_inode_wrapper));

        Ok(new_inode_wrapper)
    }

    pub fn num_of_block_groups(&self) -> usize {
        let num_of_block_groups_from_blocks: usize =
            ((self.superblock.s_blocks_count as f32) / (self.superblock.s_blocks_per_group as f32)).ceil() as usize;
        let num_of_block_groups_from_inodes: usize =
            ((self.superblock.s_inodes_count as f32) / (self.superblock.s_inodes_per_group as f32)).ceil() as usize;

        assert_eq!(num_of_block_groups_from_blocks, num_of_block_groups_from_inodes);

        num_of_block_groups_from_blocks
    }
}

impl INodeWrapper
{
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
    
    pub fn update_size(&mut self, new_size: u64) {
        self.inode.i_size = ((new_size << 32) >> 32) as u32;
        self.inode.i_dir_acl = (new_size >> 32) as u32;
    }
    
    pub fn get_deferred_write_inode<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>,
                                                    deferred_write_map: &mut DeferredWriteMap) ->
                                                               Result<(), Ext2Error> {
        let inode_block_info: INodeBlockInfo =
            Ext2::get_block_that_has_inode(&mut ext2.device, &ext2.superblock,
                                           &ext2.block_group_descriptor_tables,
                                           self.inode_num as usize);
        let inode_bytes = bytemuck::bytes_of(&self.inode);

        ext2.add_write_to_deferred_writes_map(deferred_write_map, inode_block_info.block_num,
                                              inode_block_info.block_offset, inode_bytes, None)?;

        Ok(())
    }

    pub fn get_block_group_index<D: BlockDevice>(&self, ext2: &Ext2<D>) -> usize {
        (self.inode_num / ext2.superblock.s_inodes_per_group) as usize
    }
    
    pub fn block_allocated_count<D: BlockDevice>(&self, ext2: &Ext2<D>) -> usize {
        (self.inode.i_blocks / (2 << ext2.superblock.s_log_block_size)) as usize
    }
    
    pub fn set_block_allocated_count<D: BlockDevice>(&mut self, ext2: &Ext2<D>, blocks: usize) {
        self.inode.i_blocks = (blocks as u32) * (2 << ext2.superblock.s_log_block_size);
    }

    fn get_word(byte_array: &[u8]) -> u32 {
        (byte_array[0] as u32) | ((byte_array[1] as u32) << 8) | ((byte_array[2] as u32) << 16) |
        ((byte_array[3] as u32) << 24)
    }

    fn get_half_word(byte_array: &[u8]) -> u32 {
        (byte_array[0] as u32) | ((byte_array[1] as u32) << 8)
    }

    pub fn get_inode_block_num<D: BlockDevice>(&self, number: usize, ext2: &mut Ext2<D>) -> u32 {
        let block_inode_list_size: usize = BLOCK_SIZE / size_of::<u32>();
        let block_inode_list_size_squared: usize = block_inode_list_size * block_inode_list_size;
        let block_inode_list_size_cubed: usize = block_inode_list_size_squared * block_inode_list_size;

        let mut logical_block_number: u32 = 0;
        let mut block_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];

        const TRIPLE_LINK_BLOCK_PTR_INDEX: usize = 14;
        const DOUBLE_LINK_BLOCK_PTR_INDEX: usize = 13;
        const SINGLE_LINK_BLOCK_PTR_INDEX: usize = 12;
        
        if(number >= (12 + block_inode_list_size + block_inode_list_size_squared)) {
            // hard mode: go through link to list of link of list of links to list of direct
            // block ptrs

            ext2.read_logical_block_self(self.inode.i_block[TRIPLE_LINK_BLOCK_PTR_INDEX] as usize,
                                         1, &mut block_buffer);

            let second_level_base_num: usize =
                number - (12 + block_inode_list_size + block_inode_list_size_squared);
            let index: usize =
                (second_level_base_num / block_inode_list_size_squared) * size_of::<u32>();
            let block_second_level_index: u32 =
                Self::get_word(&block_buffer[index..index+4]);

            ext2.read_logical_block_self(block_second_level_index as usize, 1, 
                                         &mut block_buffer);

            let first_level_base_num: usize = second_level_base_num % block_inode_list_size_squared;
            let block_buffer_second_index: usize =
                (first_level_base_num / block_inode_list_size) * size_of::<u32>();
            let block_first_level_index = 
                Self::get_word(
                    &block_buffer[block_buffer_second_index..block_buffer_second_index+4]);

            ext2.read_logical_block_self(block_first_level_index as usize, 1, 
                                         &mut block_buffer);

            let block_buffer_first_index: usize =
                (first_level_base_num % block_inode_list_size) * size_of::<u32>();

            logical_block_number = Self::get_word(
                &block_buffer[block_buffer_first_index..block_buffer_first_index+4]);
        } else if(number >= 12 + block_inode_list_size) {
            // medium: go through link to list of links to list of direct block ptrs
            ext2.read_logical_block_self(self.inode.i_block[DOUBLE_LINK_BLOCK_PTR_INDEX] as usize,
                                         1, &mut block_buffer);

            let first_level_base_num: usize = number - (12 + block_inode_list_size);
            let index: usize = (first_level_base_num / block_inode_list_size) * size_of::<u32>();
            let block_first_level_index: usize =
                Self::get_word(&block_buffer[index..index+4]) as usize;
            let block_final_level_index: usize =
                (first_level_base_num % block_inode_list_size) * size_of::<u32>();

            ext2.read_logical_block_self(block_first_level_index, 1, 
                                         &mut block_buffer);
            
            logical_block_number = 
                Self::get_word(&block_buffer[block_final_level_index..block_final_level_index+4]);
        } else if(number >= 12) {
            // fairly easy: go through link to list of direct block ptrs
            ext2.read_logical_block_self(self.inode.i_block[SINGLE_LINK_BLOCK_PTR_INDEX] as usize,
                                         1, &mut block_buffer);

            let index: usize = number - 12;
            let offset: usize = index * size_of::<u32>();

            logical_block_number = Self::get_word(&block_buffer[offset..offset+4]);
        } else {
            // easy: go through direct block ptrs
            logical_block_number = self.inode.i_block[number];
        }

        logical_block_number
    }

    pub fn read_block<D: BlockDevice>(&self, logical_block_start: usize,
                                      logical_block_length: usize, buffer: &mut [u8],
                                      ext2: &mut Ext2<D>) -> Result<(), Ext2Error> {
        // TODO: caching
        let mut block_tmp_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];

        for i in 0..logical_block_length {
            let cur_file_block: usize = logical_block_start + (i as usize);
            let logical_block_num: usize = self.get_inode_block_num(cur_file_block, ext2) as usize;

            ext2.read_logical_block_self(logical_block_num, 1, &mut block_tmp_buffer)?;

            for j in 0..BLOCK_SIZE {
                buffer[(i*BLOCK_SIZE)+j] = block_tmp_buffer[j];
            }
        }

        Ok(())
    }

    pub fn read_file<D: BlockDevice>(&self, ext2: &mut Ext2<D>) -> Result<Vec<u8>, Ext2Error> {
        let mut return_value: Vec<u8> = Vec::new();
        let mut blocks_to_read: usize = (self.size() as usize) / BLOCK_SIZE;

        if (self.size() as usize) % BLOCK_SIZE > 0 {
            blocks_to_read += 1;
        }

        return_value.resize(blocks_to_read * BLOCK_SIZE,0);

        self.read_block(0, blocks_to_read, return_value.as_mut_slice(), ext2)?;
        
        return_value.resize(self.size() as usize, 0);

        Ok(return_value)
    }

    pub fn read_text_file_as_str<D: BlockDevice>(&self, ext2: &mut Ext2<D>) -> 
                                                                         Result<String, Ext2Error> {
        let mut bytes: Vec<u8> = self.read_file(ext2)?;

        Ok(String::from_utf8_lossy(bytes.as_mut_slice()).into_owned())
    }

    pub fn get_dir_entries<D: BlockDevice>(&self, ext2: &mut Ext2<D>) -> Vec<DirectoryEntry> {
        // TODO: caching
        let mut entries: Vec<DirectoryEntry> = Vec::new();
        let mut entries_raw_bytes: Vec<u8> = Vec::new();
        let dir_size: usize = self.size() as usize;

        entries_raw_bytes.resize(dir_size / 9, 0);

        let directory_entry_size_blocks: usize = self.block_allocated_count(ext2);

        entries_raw_bytes.resize(directory_entry_size_blocks * BLOCK_SIZE,
                                 0);

        self.read_block(0, directory_entry_size_blocks,
                        entries_raw_bytes.as_mut_slice(), ext2);

        let mut i: usize = 0;

        while i < dir_size {
            let directory_entry_inode_num = Self::get_word(&entries_raw_bytes[i..]);
            let directory_entry_size = Self::get_half_word(&entries_raw_bytes[i+4..i+6]);
            let directory_entry_name_length =
                Self::get_half_word(&entries_raw_bytes[i+6..i+8]) as u16;
            let directory_entry_name: &[u8] =
                &entries_raw_bytes[i+8..i+8+(directory_entry_name_length as usize)];

            if directory_entry_inode_num == 0 {
                i += 8;
            }

            entries.push(DirectoryEntry{
                // all part of the spec
                inode_number: directory_entry_inode_num,
                entry_size: directory_entry_size as u16,
                name_length: directory_entry_name_length,

                name_characters: [0; DirectoryEntryConstants::MAX_FILE_NAME_LEN]
            });

            entries.iter_mut().next_back().unwrap().
                name_characters[0..directory_entry_name_length as usize].
                copy_from_slice(directory_entry_name);

            i += directory_entry_size as usize;
        }
        entries
    }
    
    pub fn find_new_blocks<D: BlockDevice>(&self, ext2: &mut Ext2<D>,
                                           num_of_blocks: usize,
                                           all_blocks_or_fail: bool,
                                           deferred_writes: &mut DeferredWriteMap)
                                           -> Result<Vec<usize>, Ext2Error> {
        let num_of_block_groups: usize = ext2.num_of_block_groups();
        let num_of_blocks_per_block_group: usize =
            ext2.superblock.s_blocks_per_group as usize;
        let mut blocks_needed_for_block_bitmap: usize =
            (num_of_blocks_per_block_group / 8) / BLOCK_SIZE;

        if (num_of_blocks_per_block_group / 8) % BLOCK_SIZE != 0 {
            blocks_needed_for_block_bitmap += 1;
        }

        let mut current_block_group_index: usize = self.get_block_group_index(ext2);
        let mut num_of_blocks_left: usize = num_of_blocks;
        let mut return_value: Vec<usize> = Vec::with_capacity(num_of_blocks);

        while num_of_blocks_left > 0 {
            // TODO: write to block_group_descriptor_tables
            let current_block_group: &BGD =
                &ext2.block_group_descriptor_tables[current_block_group_index];

            let mut free_block_count = current_block_group.bg_free_blocks_count as usize;

            while free_block_count == 0 {
                current_block_group_index =
                    (current_block_group_index + 1) % num_of_block_groups;
                free_block_count = current_block_group.bg_free_blocks_count as usize;

                if current_block_group_index == self.get_block_group_index(ext2) {
                    assert!(free_block_count == 0);

                    // we looped around all the block groups, which means there are no more
                    // remaining blocks on this filesystem :(
                    return Err(Ext2Error::BlockDeviceError(BlockDeviceError::Unknown));
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
                let mut block_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
                let mut block_buffer_dirty: bool = false;
                let mut byte_write: [u8; 1] = [0; 1];
                let mut byte_write_pos: usize = 0;

                ext2.read_logical_block_self(current_block_bitmap_block, 1,
                                             &mut block_buffer)?;

                for (index, block_buffer_byte) in block_buffer.iter_mut().enumerate() {
                    let base_block_index: usize = block_group_base_index + (index * 8);

                    for i in 0..8 {
                        if (*block_buffer_byte & (1 << i)) == 0 {
                            *block_buffer_byte |= 1 << i;
                            block_buffer_dirty = true;
                            byte_write_pos = index;
                            byte_write[0] = *block_buffer_byte;
                            blocks_allocated_from_block_group += 1;

                            return_value.push(base_block_index + i);

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
                    ext2.add_write_to_deferred_writes_map(deferred_writes, current_block_bitmap_block,
                                                          byte_write_pos, &byte_write,
                                                          Some(block_buffer))?;
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
        let mut block_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
        let mut byte_write_pos: Option<usize> = None;
        let mut byte_write: [u8; size_of::<u32>()] = [0; size_of::<u32>()];
        let double_indirect_block_block_limit: usize = BLOCK_SIZE / size_of::<u32>();

        ext2.read_logical_block_self(block_num, 1, &mut block_buffer)?;

        {
            let mut block_buffer_u32_slice: &mut [u32] =
                bytemuck::cast_slice_mut::<_, u32>(&mut block_buffer);

            // handling the contained lists of inode blocks
            for i in blocks_allocated_within_list..double_indirect_block_block_limit {
                assert!(block_buffer_u32_slice[i] == 0);

                block_buffer_u32_slice[i] = new_blocks[*blocks_newly_allocated] as u32;
                *num_of_blocks_allocated += 1;
                *blocks_newly_allocated += 1;
                byte_write_pos = Some(i * size_of::<u32>());

                if *blocks_newly_allocated == new_blocks.len() {
                    break;
                }
            }
        }

        if byte_write_pos.is_some() {
            let unwrapped_write_pos: usize = byte_write_pos.unwrap();

            byte_write.copy_from_slice(
                &block_buffer[unwrapped_write_pos..unwrapped_write_pos+size_of::<u32>()]);


            ext2.add_write_to_deferred_writes_map(deferred_write_map, self.inode.i_block[13] as usize,
                                                  unwrapped_write_pos, &byte_write,
                                                  Some(block_buffer))?;
        }

        Ok(())
    }

    fn write_new_blocks_to_inode<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>,
                                                 new_blocks: &[usize],
                                                 all_blocks_or_fail: bool,
                                                 deferred_writes: &mut DeferredWriteMap)
                                                 -> Result<usize, Ext2Error> {
        const UNALLOCATED_BLOCK_SLOT: u32 = 0;
        // assumption: references to block zero means unallocated block slot
        // TODO: write inode to disk
        // TODO: write new block num to inode
        // need to handle unallocated blocks containing doubly linked inode blocks
        // and singly linked inode blocks
        let inline_block_block_limit: usize = 12;
        let double_indirect_block_block_limit: usize = BLOCK_SIZE / size_of::<u32>();
        let triple_indirect_block_block_limit: usize =
            double_indirect_block_block_limit * double_indirect_block_block_limit;
        let mut num_of_blocks_allocated: usize = self.block_allocated_count::<D>(ext2);

        let mut blocks_newly_allocated: usize = 0;

        if triple_indirect_block_block_limit + double_indirect_block_block_limit +
            inline_block_block_limit < num_of_blocks_allocated {
            // we've run out of places to put blocks in the inode which means
            // we've reached the largest file size limit
            return Ok(0);
        }

        if num_of_blocks_allocated < inline_block_block_limit &&
            blocks_newly_allocated < new_blocks.len()  {
            for i in num_of_blocks_allocated..12 {
                assert!(self.inode.i_block[i] == UNALLOCATED_BLOCK_SLOT);

                self.inode.i_block[i] = new_blocks[blocks_newly_allocated] as u32;
                num_of_blocks_allocated += 1;
                blocks_newly_allocated += 1;

                if blocks_newly_allocated == new_blocks.len() {
                    break;
                }
            }
        }

        if num_of_blocks_allocated < double_indirect_block_block_limit + inline_block_block_limit &&
            blocks_newly_allocated < new_blocks.len() {
            let mut block_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];

            if self.inode.i_block[13] == UNALLOCATED_BLOCK_SLOT {
                let new_blocks_allocated: Vec<usize> =
                    self.find_new_blocks::<D>(ext2, 1, all_blocks_or_fail,
                                              deferred_writes)?;

                if new_blocks_allocated.is_empty() {
                    self.set_block_allocated_count(ext2, 
                        self.block_allocated_count(ext2) + blocks_newly_allocated);

                    return Ok(blocks_newly_allocated);
                }

                self.inode.i_block[13] = new_blocks[0] as u32;
            }

            if self.inode.i_block[13] != UNALLOCATED_BLOCK_SLOT {
                ext2.read_logical_block_self(self.inode.i_block[13] as usize,
                                             1, &mut block_buffer)?;

                let mut byte_write_pos: usize = 0;

                {
                    let block_list_u32_slice: &mut [u32] =
                        bytemuck::cast_slice_mut::<_, u32>(&mut block_buffer);

                    for i in num_of_blocks_allocated..(num_of_blocks_allocated + double_indirect_block_block_limit) {
                        assert!(block_list_u32_slice[i] == UNALLOCATED_BLOCK_SLOT);

                        block_list_u32_slice[i] = new_blocks[blocks_newly_allocated] as u32;
                        num_of_blocks_allocated += 1;
                        blocks_newly_allocated += 1;
                        byte_write_pos = i * size_of::<u32>();

                        if blocks_newly_allocated == new_blocks.len() {
                            break;
                        }
                    }
                }

                ext2.add_write_to_deferred_writes_map(deferred_writes, self.inode.i_block[13] as usize,
                                                      byte_write_pos,
                                                      &block_buffer[byte_write_pos..size_of::<u32>()],
                                                      Some(block_buffer))?;
            }
        }

        if blocks_newly_allocated < new_blocks.len() {
            let count_of_doubly_linked_blocks: usize = num_of_blocks_allocated -
                (inline_block_block_limit + double_indirect_block_block_limit);

            let starting_doubly_linked_block: usize =
                count_of_doubly_linked_blocks / double_indirect_block_block_limit;
            let ending_doubly_linked_block: usize =
                std::cmp::min(double_indirect_block_block_limit,
                              new_blocks.len() - blocks_newly_allocated);
            let mut new_blocks_needed_for_block_list_count: usize = 0;

            // find all blocks needed to complete the write
            if self.inode.i_block[14] == UNALLOCATED_BLOCK_SLOT {
                let new_block_list_blocks_result =
                    self.find_new_blocks(ext2, 1, all_blocks_or_fail, deferred_writes);

                if new_block_list_blocks_result.is_err() {
                    return Err(new_block_list_blocks_result.unwrap_err());
                }

                let new_block_list: Vec<usize> = new_block_list_blocks_result?;

                if new_block_list.len() == 0 {
                    self.set_block_allocated_count(ext2,
                                                   self.block_allocated_count(ext2) + blocks_newly_allocated);
                    return Ok(blocks_newly_allocated);
                }

                self.inode.i_block[14] = new_block_list[0] as u32;
            }

            let mut block_list_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];

            ext2.read_logical_block_self(self.inode.i_block[14] as usize,
                                         1, &mut block_list_buffer)?;

            let block_list_u32_slice: &mut [u32] =
                bytemuck::cast_slice_mut::<_, u32>(&mut block_list_buffer);

            for block_num in starting_doubly_linked_block..ending_doubly_linked_block {
                if block_list_u32_slice[block_num] == UNALLOCATED_BLOCK_SLOT {
                    new_blocks_needed_for_block_list_count += 1;
                }
            }

            let new_block_list_blocks_result =
                self.find_new_blocks(ext2,new_blocks_needed_for_block_list_count,
                                     all_blocks_or_fail, deferred_writes);

            if new_block_list_blocks_result.is_err() {
                return Err(new_block_list_blocks_result.unwrap_err());
            }
            
            let new_block_list_blocks: Vec<usize> = new_block_list_blocks_result?;
            let mut new_block_list_index: usize = 0;

            assert!(!all_blocks_or_fail || 
                    new_block_list_blocks.len() == new_blocks_needed_for_block_list_count);

            for block_num in starting_doubly_linked_block..ending_doubly_linked_block {
                let num_of_blocks_allocated_within_list =
                    num_of_blocks_allocated % double_indirect_block_block_limit;

                if block_list_u32_slice[block_num] == UNALLOCATED_BLOCK_SLOT {
                    if new_block_list_index >= new_block_list_blocks.len() {
                        self.set_block_allocated_count(ext2,
                                                       self.block_allocated_count(ext2) + blocks_newly_allocated);
                        return Ok(blocks_newly_allocated);
                    }

                    block_list_u32_slice[block_num] =
                        new_block_list_blocks[new_block_list_index] as u32;
                    new_block_list_index += 1;
                }

                self.write_indirected_block_to_inode(ext2, block_list_u32_slice[block_num] as usize,
                                                     num_of_blocks_allocated_within_list,
                                                     &mut num_of_blocks_allocated,
                                                     &mut blocks_newly_allocated, new_blocks,
                                                     deferred_writes)?;
            }

            if !new_block_list_blocks.is_empty() {
                self.set_block_allocated_count(ext2,
                                               self.block_allocated_count(ext2) + blocks_newly_allocated);
                return Ok(blocks_newly_allocated);
            }
        }

        self.set_block_allocated_count(ext2,
                                       self.block_allocated_count(ext2) + blocks_newly_allocated);

        Ok(blocks_newly_allocated)
    }

    fn div_up(a: usize, b: usize) -> usize {
        (a / b) + (if a % b > 0 { 1 } else {0})
    }

    fn append_file_no_writeback<D: BlockDevice>(&mut self, ext2: &mut Ext2<D>, new_data: &[u8],
                                all_bytes_or_fail: bool,
                                deferred_writes: &mut DeferredWriteMap) -> Result<usize, Ext2Error> {
        let allocated_block_count: usize = self.block_allocated_count(ext2);
        let base_allocated_block: Option<usize> =
            if allocated_block_count == 0 { None } else {
                Some(self.get_inode_block_num(allocated_block_count - 1, ext2) as usize)
            };

        let base_allocated_block_offset: usize = (self.size() as usize) % BLOCK_SIZE;
        let mut new_blocks_allocated: Vec<usize> = Vec::new();
        let mut bytes_written: usize = 0;

        if base_allocated_block_offset > 0 {
            new_blocks_allocated.push(base_allocated_block.unwrap());
        }

        if base_allocated_block_offset == 0 ||
           (BLOCK_SIZE - base_allocated_block_offset) < new_data.len() {
            let remaining_block_slots_left =
                ext2.block_group_descriptor_tables[self.get_block_group_index(ext2)].bg_free_blocks_count;

            let num_of_blocks_needed: usize =
                std::cmp::min(remaining_block_slots_left as usize,
                              Self::div_up(new_data.len(), BLOCK_SIZE));

            let new_blocks_allocated_result =
                self.find_new_blocks::<D>(ext2, num_of_blocks_needed, all_bytes_or_fail,
                                          deferred_writes);

            new_blocks_allocated.append(&mut new_blocks_allocated_result?);

            self.write_new_blocks_to_inode(ext2,
                                           &new_blocks_allocated[new_blocks_allocated.len() - 1..],
                             true, deferred_writes)?;
        }

        for new_block in new_blocks_allocated {
            let write_base = if base_allocated_block.is_some() && new_block == base_allocated_block.unwrap() {
                base_allocated_block_offset
            } else {
                0
            };
            let write_size = std::cmp::min(if base_allocated_block.is_some() && new_block == base_allocated_block.unwrap() {
                BLOCK_SIZE - base_allocated_block_offset
            } else {
                BLOCK_SIZE
            }, new_data.len() - bytes_written);

            let current_byte_slice: &[u8] = &new_data[bytes_written..bytes_written+write_size];

            bytes_written += write_size;

            ext2.add_write_to_deferred_writes_map(deferred_writes, new_block, write_base,
                                                  current_byte_slice, None)?;
        }

        self.update_size(self.size() + (bytes_written as u64));
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

    // overwrite over file, with the new file size being the size of new_data
    pub fn overwrite_file<D: BlockDevice>(&self, ext2: &mut Ext2<D>, new_data: &[u8],
                                          all_bytes_or_fail: bool) -> Result<usize, Ext2Error> {
        todo!();
    }
}
