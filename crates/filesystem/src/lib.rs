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
use alloc::rc::Rc;
use std::fs::DirEntry;
use std::prelude::v1::{String, Vec};

#[cfg(test)]
mod tests;

#[cfg(feature = "std")]
pub mod linux;

pub const SECTOR_SIZE: usize = 512;
pub const BLOCK_SIZE: usize = 1024;

pub enum BlockDeviceError {
    Unknown,
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
        let mut tmp_buf: [u8; 512] = [0; 512];
        for i in 0..sectors {
            let cur_sector = start_index + (i as u64);
            for j in 0..SECTOR_SIZE {
                buffer[(i*SECTOR_SIZE)+j] = tmp_buf[j];
            }
            self.read_sector(cur_sector, &mut tmp_buf)?;
        }
        Ok(())
    }
}

pub struct Ext2<Device> {
    device: Device,
    superblock: Superblock,
    block_group_descriptor_tables: Vec<BGD>,
    root_inode: Rc<INodeWrapper>
}

struct DirectoryEntry {
    // assumption: name length <= 2^16
    // all part of the spec
    inode_number: u32,
    entry_size: u16,
    name_length: u16,

    name_characters: String
}
// https://www.nongnu.org/ext2-doc/ext2.html

#[repr(C)]
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
    unused_alignment_3: [u8;760],
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
#[derive(Clone)]
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
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
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

pub mod reserved_inodes {
    const EXT2_BAD_INO: u32 = 1;
    const EXT2_ROOT_INO: u32 = 2;
    const EXT2_ACL_IDX_INO: u32 = 3;
    const EXT2_ACL_DATA_INO: u32 = 4;
    const EXT2_BOOT_LOADER_INO: u32 = 5;
    const EXT2_UNDEL_DIR_INO: u32 = 6;
}

pub mod i_mode {
    const EXT2_S_IFSOCK: u16 = 0xC000;
    pub(crate) const EXT2_S_IFLNK: u16 = 0xA000;
    const EXT2_S_IFREG: u16 = 0x8000;
    const EXT2_S_IFBLK: u16 = 0x6000;
    pub const EXT2_S_IFDIR: u16 = 0x4000;
    const EXT2_S_IFCHR: u16 = 0x2000;
    const EXT2_S_IFIFO: u16 = 0x1000;
    const EXT2_S_ISUID: u16 = 0x0800;
    const EXT2_S_ISGID: u16 = 0x0400;
    const EXT2_S_ISVTX: u16 = 0x0200;
    const EXT2_S_IRUSR: u16 = 0x0100;
    const EXT2_S_IWUSR: u16 = 0x0080;
    const EXT2_S_IXUSR: u16 = 0x0040;
    const EXT2_S_IRGRP: u16 = 0x0020;
    const EXT2_S_IWGRP: u16 = 0x0010;
    const EXT2_S_IXGRP: u16 = 0x0008;
    const EXT2_S_IROTH: u16 = 0x0004;
    const EXT2_S_IWOTH: u16 = 0x0002;
    const EXT2_S_IXOTH: u16 = 0x0001;
}

pub mod i_flags {
    const EXT2_SECRM_FL: u32 = 0x00000001;
    const EXT2_UNRM_FL: u32 = 0x00000002;
    const EXT2_COMPR_FL: u32 = 0x00000004;
    const EXT2_SYNC_FL: u32 = 0x00000008;
    const EXT2_IMMUTABLE_FL: u32 = 0x00000010;
    const EXT2_APPEND_FL: u32 = 0x00000020;
    const EXT2_NODUMP_FL: u32 = 0x00000040;
    const EXT2_NOATIME_FL: u32 = 0x00000080;
    const EXT2_DIRTY_FL: u32 = 0x00000100;
    const EXT2_COMPRBLK_FL: u32 = 0x00000200;
    const EXT2_NOCOMPR_FL: u32 = 0x00000400;
    const EXT2_ECOMPR_FL: u32 = 0x00000800;
    const EXT2_BTREE_FL: u32 = 0x00001000;
    const EXT2_INDEX_FL: u32 = 0x00001000;
    const EXT2_IMAGIC_FL: u32 = 0x00002000;
    const EXT3_JOURNAL_DATA_FL: u32 = 0x00004000;
    const EXT2_RESERVED_FL: u32 = 0x80000000;
}

const _: () = assert!(size_of::<INode>() == 128);

pub mod file_type {
    const EXT2_FT_UNKNOWN: u8 = 0;
    const EXT2_FT_REG_FILE: u8 = 1;
    const EXT2_FT_DIR: u8 = 2;
    const EXT2_FT_CHRDEV: u8 = 3;
    const EXT2_FT_BLKDEV: u8 = 4;
    const EXT2_FT_FIFO: u8 = 5;
    const EXT2_FT_SOCK: u8 = 6;
    const EXT2_FT_SYMLINK: u8 = 7;
}

pub struct INodeWrapper {
    inode: INode,

    inode_num: u32,
    inode_block_index: usize,
    inode_block_offset: usize,
    block_group_index: usize,
    remaining_block_slots_left: usize
}

pub struct DeferredWrite {
    buffer: [u8; BLOCK_SIZE],
    block_num: usize
}

impl<D> Ext2<D>
where
    D: BlockDevice,
{
    fn read_logical_block(device: &mut D, logical_block_start: usize, logical_block_length: usize,
                          buffer: &mut [u8]) {
        assert!(logical_block_length > 0);

        let start_sector_numerator: usize = logical_block_start * BLOCK_SIZE;
        let start_sector: usize = start_sector_numerator / SECTOR_SIZE;
        let sectors: usize = (logical_block_length * BLOCK_SIZE) / SECTOR_SIZE;

        device.read_sectors(start_sector as u64, sectors, buffer);
    }

    fn write_logical_block(device: &mut D, logical_block_start: usize, logical_block_length: usize,
                           buffer: &mut [u8]) {
        assert!(logical_block_length > 0);

        let start_sector_numerator: usize = logical_block_start * BLOCK_SIZE;
        let start_sector: usize = start_sector_numerator / SECTOR_SIZE;
        let sectors: usize = (logical_block_length * BLOCK_SIZE) / SECTOR_SIZE;

        device.write_sector(start_sector as u64, sectors, buffer);
    }

    pub fn write_logical_block_self(&mut self, logical_block_start: usize,
                                    logical_block_length: usize, buffer: &mut [u8]) {
        Self::write_logical_block(&mut self.device, logical_block_start, logical_block_length,
                                  buffer);
    }

    pub fn read_logical_block_self(&mut self, logical_block_start: usize, logical_block_length: usize,
                                   buffer: &mut [u8]) {
        Self::read_logical_block(&mut self.device, logical_block_start, logical_block_length, 
                                 buffer);
    }
 
    fn get_inode(device: &mut D, superblock: &Superblock, block_group_descriptor_tables: &Vec<BGD>,
                 inode_num: usize) -> INode {
        let inode_size = superblock.s_inode_size as usize;

        let block_group_number = (inode_num - 1) / superblock.s_inodes_per_group as usize;
        let inode_table_block =
            block_group_descriptor_tables[block_group_number].bg_inode_table as usize;

        let inode_table_index: usize = (inode_num - 1) % (superblock.s_inodes_per_group as usize);
        let inode_table_block_with_offset: usize =
            ((inode_table_index * inode_size) / BLOCK_SIZE) + inode_table_block;
        let inode_table_interblock_offset: usize = (inode_table_index * inode_size) % BLOCK_SIZE;

        let mut block_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];

        Self::read_logical_block(device, inode_table_block_with_offset, 1,
                                 &mut block_buffer);

        let mut inode_data: [u8; size_of::<INode>()] = [0x00; size_of::<INode>()];

        inode_data.copy_from_slice(&block_buffer[inode_table_interblock_offset..inode_table_interblock_offset + size_of::<INode>()]);

        let inode: INode =
            unsafe {std::mem::transmute::<[u8;size_of::<INode>()], INode>(inode_data)};

        inode
    }

    pub fn get_root_inode_wrapper(&mut self) -> Rc<INodeWrapper> {
        self.root_inode.clone()
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

        Self { device, superblock, block_group_descriptor_tables, 
               root_inode: Rc::new(INodeWrapper{ inode: root_inode, inode_num: 2 }) }
    }

    pub fn get_block_size(&mut self) -> u32 {
        1024 << self.superblock.s_log_block_size
    }

    pub fn get_inode_size(&mut self) -> u32 {
        self.superblock.s_inode_size as u32
    }

    pub fn find(&mut self, node: &INodeWrapper, name: &str) -> Option<Rc<INodeWrapper>> {
        let dir_entries: Vec<DirectoryEntry> = node.get_dir_entries(self);

        if node.is_dir() {
            for dir_entry in dir_entries {
                if(dir_entry.name_characters == name) {
                    // TODO: find out how to do operator overloading in rust and convert this into
                    // TODO: a mut self method
                    let inode: INode = Self::get_inode(&mut self.device, &self.superblock,
                                                       &self.block_group_descriptor_tables,
                                                       dir_entry.inode_number as usize);

                    return Some(Rc::new(INodeWrapper { inode, inode_num: dir_entry.inode_number }));
                }
            }
        }

        None
    }

    fn acquire_next_available_inode(&mut self, inode_data: INode) -> Option<Rc<INodeWrapper>> {
        for (block_group_index, mut block_group_table)
        in self.block_group_descriptor_tables.iter_mut().enumerate() {
            if block_group_table.bg_free_inodes_count > 1 {
                block_group_table.bg_free_inodes_count -= 1;

                let mut block_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];

                self.read_logical_block_self(block_group_table.bg_block_bitmap as usize,
                                             1, &mut block_buffer);

                let mut found_new_inode: bool = false;
                let mut new_inode_num: usize =
                    (self.superblock.s_inodes_per_group as usize) * block_group_index;

                for (inode_bitmap_byte_index, inode_bitmap_byte)
                  in block_buffer.iter().enumerate() {
                    for i in 0..8 {
                        if(inode_bitmap_byte & (1 << (7 - i)) == 0) {
                            new_inode_num = (inode_bitmap_byte_index * 8) + i;
                            found_new_inode = true;
                            break;
                        }
                    }

                    if found_new_inode {
                        break;
                    }
                }

                assert!(found_new_inode);

                let num_of_inodes_per_block: usize = BLOCK_SIZE / size_of::<INode>();
                let inode_block_index: usize =
                    ((block_group_table.bg_inode_table as usize) +
                     ((new_inode_num - 1) / num_of_inodes_per_block));
                let inode_block_offset: usize = (new_inode_num - 1) % num_of_inodes_per_block;

                self.read_logical_block_self(inode_block_index, 1,
                                             &mut block_buffer);

                let inode_bytes = bytemuck::bytes_of(&inode_data);

                block_buffer[inode_block_offset..inode_block_offset + size_of::<INode>()].copy_from_slice(inode_bytes);

                return Some(Rc::new(INodeWrapper{inode: inode_data,
                                                       inode_num: new_inode_num as u32,
                                                       inode_block_index: inode_block_index, 
                                                       inode_block_offset: inode_block_offset}));
            }
        }
        None
    }

    // Creates a file named name (<= 255 characters),
    // returns None if out of disk space
    pub fn create_file(&mut self, node: &INodeWrapper, name: &str,
                       ext2: &mut Ext2<D>) -> Result<Rc<INodeWrapper>, std::io::Error> {
        // what do we need to do when creating a new file?
        // go thru BGD inode bitmaps, find the next unallocated inode number and update it
        // update inode number
        // add a directory entry pointing to our inode thru append_file
        let new_inode_option = self.acquire_next_available_inode();

        if(new_inode_option == None) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid file"));
        }

        let new_inode = new_inode_option.unwrap();



        Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid file"))
    }

    pub fn num_of_block_groups(&self) -> usize {
        let num_of_block_groups_from_blocks: usize =
            (self.superblock.s_blocks_count / self.superblock.s_blocks_per_group) as usize;
        let num_of_block_groups_from_inodes: usize =
            (self.superblock.s_inodes_count / self.superblock.s_inodes_per_group) as usize;

        assert!(num_of_block_groups_from_blocks == num_of_block_groups_from_inodes);

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
                                      ext2: &mut Ext2<D>) {
        // TODO: caching
        let mut block_tmp_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];

        for i in 0..logical_block_length {
            let cur_file_block: usize = logical_block_start + (i as usize);
            let logical_block_num: usize = self.get_inode_block_num(cur_file_block, ext2) as usize;

            ext2.read_logical_block_self(logical_block_num, 1, &mut block_tmp_buffer);

            for j in 0..BLOCK_SIZE {
                buffer[(i*BLOCK_SIZE)+j] = block_tmp_buffer[j];
            }
        }
    }

    pub fn read_file<D: BlockDevice>(&self, ext2: &mut Ext2<D>) -> Vec<u8> {
        let mut return_value: Vec<u8> = Vec::new();
        let mut blocks_to_read: usize = (self.size() as usize) / BLOCK_SIZE;

        if (self.size() as usize) % BLOCK_SIZE > 0 {
            blocks_to_read += 1;
        }

        return_value.resize(blocks_to_read * BLOCK_SIZE,0);

        self.read_block(0, blocks_to_read, return_value.as_mut_slice(), ext2);

        return_value
    }

    pub fn read_text_file_as_str<D: BlockDevice>(&self, ext2: &mut Ext2<D>) -> String {
        let mut bytes: Vec<u8> = self.read_file(ext2);

        String::from_utf8_lossy(bytes.as_mut_slice()).into_owned().trim_end_matches('\0').into()
    }

    pub fn get_dir_entries<D: BlockDevice>(&self, ext2: &mut Ext2<D>) -> Vec<DirectoryEntry> {
        // TODO: caching
        let mut entries: Vec<DirectoryEntry> = Vec::new();
        let mut entries_raw_bytes: Vec<u8> = Vec::new();
        let dir_size: usize = self.size() as usize;

        entries_raw_bytes.resize(dir_size / 9, 0);

        let directory_entry_size_blocks: usize = dir_size / BLOCK_SIZE;

        entries_raw_bytes.resize(directory_entry_size_blocks * BLOCK_SIZE,
                                 0);

        self.read_block(0, directory_entry_size_blocks,
                        entries_raw_bytes.as_mut_slice(), ext2);

        let mut i: usize = 0;

        while i < dir_size {
            let directory_entry_inode_num = Self::get_word(&entries_raw_bytes[i..]);
            let directory_entry_size = Self::get_half_word(&entries_raw_bytes[i+4..i+6]);
            let directory_entry_name_length = entries_raw_bytes[i+6];
            let directory_entry_name =
                String::from_utf8_lossy(&entries_raw_bytes[i+8..i+8+(directory_entry_name_length as usize)]).into_owned();

            entries.push(DirectoryEntry{
                // all part of the spec
                inode_number: directory_entry_inode_num,
                entry_size: directory_entry_size as u16,
                name_length: directory_entry_name_length as u16,

                name_characters: directory_entry_name
            });

            i += directory_entry_size as usize;
        }
        entries
    }

    pub fn find_new_blocks<D: BlockDevice>(&self, ext2: &mut Ext2<D>,
                                           num_of_blocks: usize,
                                           all_blocks_or_fail: bool) -> Result<Vec<usize>, std::io::Error> {
        let num_of_block_groups: usize = ext2.num_of_block_groups();
        let num_of_blocks_per_block_group: usize =
            ext2.superblock.s_blocks_per_group as usize;
        let mut blocks_needed_for_block_bitmap: usize =
            (num_of_blocks_per_block_group / 8) / BLOCK_SIZE;

        if (num_of_blocks_per_block_group / 8) % BLOCK_SIZE != 0 {
            blocks_needed_for_block_bitmap += 1;
        }

        let mut current_block_group_index: usize = self.block_group_index;
        let mut num_of_blocks_left: usize = num_of_blocks;
        let mut return_value: Vec<usize> = Vec::with_capacity(num_of_blocks);
        let mut deferred_writes: Vec<DeferredWrite> = Vec::new();

        while num_of_blocks_left > 0 {
            // TODO: write to block_group_descriptor_tables
            let current_block_group: &BGD =
                &ext2.block_group_descriptor_tables[current_block_group_index];

            let mut free_block_count = current_block_group.bg_free_blocks_count as usize;

            while free_block_count == 0 {
                current_block_group_index =
                    (current_block_group_index + 1) % num_of_block_groups;
                free_block_count = current_block_group.bg_free_blocks_count as usize;

                if current_block_group_index == self.inode_block_index {
                    assert!(free_block_count == 0);

                    // we looped around all the block groups, which means there are no more
                    // remaining blocks on this filesystem :(
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,
                                                   "invalid file"));
                }
            }

            let block_group_base_index: usize =
                num_of_blocks_per_block_group * current_block_group_index;
            let mut blocks_allocated_from_block_group: usize = 0;
            let mut current_block_bitmap_block: usize =
                current_block_group.bg_block_bitmap as usize;
            let last_block_bitmap_block: usize =
                current_block_bitmap_block + blocks_needed_for_block_bitmap;

            while blocks_allocated_from_block_group < free_block_count {
                let mut block_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
                let mut block_buffer_dirty: bool = false;

                ext2.read_logical_block_self(current_block_bitmap_block, 1,
                                             &mut block_buffer);

                for (index, block_buffer_byte) in block_buffer.iter().enumerate() {
                    let base_block_index: usize = block_group_base_index + (index * 8);

                    for i in 0..8 {
                        if (block_buffer_byte & (1 << i)) == 0 {
                            block_buffer_byte |= (1 << i);
                            block_buffer_dirty = true;
                            blocks_allocated_from_block_group += 1;

                            return_value.push(base_block_index + i);

                            if blocks_allocated_from_block_group >= free_block_count {
                                break;
                            }
                        }
                    }

                    if blocks_allocated_from_block_group >= free_block_count {
                        break;
                    }
                }

                if block_buffer_dirty {
                    deferred_writes.push(DeferredWrite{buffer: block_buffer,
                                                             block_num: current_block_bitmap_block});
                }

                if blocks_allocated_from_block_group < free_block_count {
                    current_block_bitmap_block += 1;

                    assert!(current_block_bitmap_block < last_block_bitmap_block);
                }
            }

            num_of_blocks_left -= free_block_count;
        }

        // write-back block changes to filesystem
        for deferred_write in deferred_writes {
            ext2.write
        }

        Ok(return_value)
    }

    fn write_new_blocks_to_inode<D: BlockDevice>(&self, ext2: &mut Ext2<D>,
                                                 new_blocks: &Vec<usize>) -> usize {
        // assumption: references to block zero means unallocated block slot
        // TODO: write inode to disk
        // need to handle unallocated blocks containing doubly linked inode blocks
        // and singly linked inode blocks
        let inline_block_block_limit: usize = 12;
        let double_indirect_block_block_limit: usize = BLOCK_SIZE / size_of::<u32>();
        let triple_indirect_block_block_limit: usize =
            double_indirect_block_block_limit * double_indirect_block_block_limit;
        let mut num_of_blocks_allocated = i_blocks/(2<<s_log_block_size);

        let mut blocks_newly_allocated: usize = 0;

        if triple_indirect_block_block_limit + double_indirect_block_block_limit + inline_block_block_limit < num_of_blocks_allocated {
            // we've run out of places to put blocks in the inode which means
            // we've reached the largest file size limit
            return 0;
        }

        if num_of_blocks_allocated < inline_block_block_limit &&
            blocks_newly_allocated < new_blocks.len()  {
            for i in num_of_blocks_allocated..12 {
                assert!(self.inode.i_block[i] == 0);

                self.inode.i_block[i] = new_blocks[blocks_newly_allocated];
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

            ext2.read_logical_block_self(self.inode.i_block[13] as usize,
                                         1, &mut block_buffer);

            for i in num_of_blocks_allocated..(num_of_blocks_allocated + double_indirect_block_block_limit) {
                assert!(block_buffer[i] == 0);

                block_buffer[i] = new_blocks[blocks_newly_allocated];
                num_of_blocks_allocated += 1;
                blocks_newly_allocated += 1;

                if blocks_newly_allocated == new_blocks.len() {
                    break;
                }
            }

            deferred_writes.push(DeferredWrite{ buffer: block_buffer,
                                                block_num: self.inode.i_block[13] as usize });
        }

        if blocks_newly_allocated < new_blocks.len() {
            let count_of_doubly_linked_blocks: usize = num_of_blocks_allocated -
                (inline_block_block_limit + double_indirect_block_block_limit);
            let mut starting_doubly_linked_block: usize =
                count_of_doubly_linked_blocks / double_indirect_block_block_limit;

            
            
            // handling the blocks pointing to lists of inode blocks
            for block_num in starting_doubly_linked_block..double_indirect_block_block_limit {
                let num_of_blocks_allocated_within_list =
                    num_of_blocks_allocated % double_indirect_block_block_limit;
                let mut block_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];

                ext2.read_logical_block_self(block_num, 1, &mut block_buffer);

                // handling the contained lists of inode blocks
                for i in num_of_blocks_allocated_within_list..double_indirect_block_block_limit {
                    assert!(block_buffer[i] == 0);

                    block_buffer[i] = new_blocks[blocks_newly_allocated];
                    num_of_blocks_allocated += 1;
                    blocks_newly_allocated += 1;

                    if blocks_newly_allocated == new_blocks.len() {
                        break;
                    }
                }

                deferred_writes.push(DeferredWrite{buffer: block_buffer, 
                                                   block_num: self.inode.i_block[13] as usize})
            }
        }

        blocks_newly_allocated
    }

    // append to file, with the new file size being the existing file size + size of new_data
    pub fn append_file<D: BlockDevice>(&self, ext2: &mut Ext2<D>, new_data: &[u8],
                                       all_blocks_or_fail: bool) -> Result<usize, std::io::Error> {
        let mut current_unallocated_block_size: usize = (self.size() as usize) % BLOCK_SIZE;
        let mut size_counter: usize = self.size() as usize;
        let needed_size: usize = (self.size() as usize) + new_data.len();
        let mut new_blocks_allocated: Vec<usize> = Vec::new();

        if current_unallocated_block_size < new_data.len() {
            let num_of_blocks_needed: usize =
                std::cmp::min(self.remaining_block_slots_left,
                              (self.size() as usize) / BLOCK_SIZE);

            let new_blocks_allocated_result =
                self.find_new_blocks::<D>(ext2, num_of_blocks_needed, all_blocks_or_fail);

            if new_blocks_allocated_result.is_err() {
                return Err(new_blocks_allocated_result.unwrap_err());
            }

            new_blocks_allocated = new_blocks_allocated_result?;
        }
    }

    // overwrite over file, with the new file size being the size of new_data
    pub fn overwrite_file<D: BlockDevice>(&self, ext2: &mut Ext2<D>, new_data: &[u8]) {
        //
    }
}
