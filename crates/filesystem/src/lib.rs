#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use std::prelude::v1::Vec;

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
}

pub struct Ext2<Device> {
    device: Device,
    superblock: Superblock,
    block_group_descriptor_tables: Vec<BGD>,
    root_inode: INode
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

    fn get_inode(device: &mut D, superblock: &Superblock, block_group_descriptor_tables: &Vec<BGD>,
                 inode: usize) -> INode {
        let inode_size = superblock.s_inode_size as usize;

        let block_group_number = (inode - 1) / superblock.s_inodes_per_group as usize;
        let inode_table_block =
            block_group_descriptor_tables[block_group_number].bg_inode_table as usize;

        let inode_table_index: usize = (inode - 1) % (superblock.s_inodes_per_group as usize);
        let inode_table_block_with_offset: usize =
            ((inode_table_index * inode_size) / BLOCK_SIZE) + inode_table_block;

        let mut block_buffer: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];

        Self::read_logical_block(device, inode_table_block_with_offset, 1,
                                 &mut block_buffer);

        let inodes_per_block: usize = BLOCK_SIZE / inode_size;
        let inode_rel_index: usize = inode_table_index % inodes_per_block;
        let block_buffer_offset: usize = inode_rel_index * inode_size;

        let mut inode_data: [u8; size_of::<INode>()] = [0x00; size_of::<INode>()];

        inode_data.copy_from_slice(&block_buffer[0..size_of::<INode>()]);

        let inode: INode =
            unsafe {std::mem::transmute::<[u8;size_of::<INode>()], INode>(inode_data)};

        inode
    }

    pub fn new(mut device: D) -> Self {
        let mut buffer: [u8; 1024] = [0; 1024];
        // todo: error-handling device.read_sectors
        device.read_sectors(1, 2, &mut buffer);
        let superblock: Superblock = unsafe { std::mem::transmute::<[u8; 1024], Superblock>(buffer) };

        let mut block_group_descriptor_tables: Vec<BGD> = Vec::new();
        let descriptor_table_ptr: *mut BGD = block_group_descriptor_tables.as_mut_ptr();

        let block_group_descriptor_block: usize = if BLOCK_SIZE == 1024 {2} else {1};
        let block_group_descriptor_length: usize =
            1 + (((superblock.get_num_of_block_groups() as usize) * size_of::<BGD>()) / BLOCK_SIZE);

        block_group_descriptor_tables.reserve(block_group_descriptor_length);

        unsafe {
            let byte_slice: &mut[u8] =
                std::slice::from_raw_parts_mut(descriptor_table_ptr as *mut u8,
                                                block_group_descriptor_length * BLOCK_SIZE);

            Self::read_logical_block(&mut device, block_group_descriptor_block,
                                     block_group_descriptor_length, byte_slice);

            block_group_descriptor_tables.set_len(superblock.get_num_of_block_groups() as usize);
        }

        let root_inode: INode =
            Self::get_inode(&mut device, &superblock, &block_group_descriptor_tables, 1);

        Self { device, superblock, block_group_descriptor_tables, root_inode }
    }

    pub fn get_block_size(&mut self) -> u32 {
        1024 << self.superblock.s_log_block_size
    }

    pub fn get_inode_size(&mut self) -> u32 {
        self.superblock.s_inode_size as u32
    }

    //pub fn find(dir: )
}

#[repr(C)]
struct DirectoryEntry {
    // assumption: name length <= 2^16
    // all part of the spec
    inode_number: u32,
    entry_size: u16,
    name_length: u16,

    // theoretically a way to do unsized arrays safely
    // ask alex for more help when we need this
    // not strictly part of the spec
    // char name_characters[];
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
    const EXT2_S_IFLNK: u16 = 0xA000;
    const EXT2_S_IFREG: u16 = 0x8000;
    const EXT2_S_IFBLK: u16 = 0x6000;
    const EXT2_S_IFDIR: u16 = 0x4000;
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

