#![no_std]
#![warn(clippy::large_stack_arrays)]

extern crate alloc;

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::ControlFlow;

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
}

pub struct Ext2<Device> {
    device: Device,
    superblock: Superblock,
    block_group_descriptor_tables: Vec<BGD>,
    root_inode: Rc<INodeWrapper>,
}

#[repr(C)]
struct DirEntryData {
    inode: u32,
    rec_len: u16,
    name_len: u8,
    file_type: u8,
    name_chars: [u8; 0],
}

pub struct DirEntry<'a> {
    pub inode_num: u32,
    pub name_length: u8,
    pub file_type: u8,
    pub name: &'a [u8],
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
    unused_alignment_3: [u8; 760],
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
#[derive(Clone)]
pub struct BGD {
    bg_block_bitmap: u32,
    bg_inode_bitmap: u32,
    bg_inode_table: u32,
    bg_free_blocks_count: u16,
    bg_free_inodes_count: u16,
    bg_used_dirs_count: u16,
    bg_pad: u16,
    bg_reserved: [u8; 12],
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
    i_block: [u32; 15], // 12 direct, single, double, triple
    i_generation: u32,
    i_file_acl: u32,
    i_dir_acl: u32,
    i_faddr: u32,
    i_osd2: [u8; 12],
}

const _: () = assert!(size_of::<INode>() == 128);

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
    _inode_num: u32,
}

impl<D> Ext2<D>
where
    D: BlockDevice,
{
    fn read_logical_block_inner(
        device: &mut D,
        block_size: usize,
        logical_block_start: usize,
        buffer: &mut [u8],
    ) -> Result<(), Ext2Error> {
        assert_eq!(buffer.len(), block_size);
        let start_sector_numerator: usize = logical_block_start * block_size;
        let start_sector: usize = start_sector_numerator / SECTOR_SIZE;
        device.read_sectors(start_sector as u64, buffer)?;
        Ok(())
    }

    pub fn read_logical_block(
        &mut self,
        logical_block_start: usize,
        buffer: &mut [u8],
    ) -> Result<(), Ext2Error> {
        Self::read_logical_block_inner(
            &mut self.device,
            self.superblock.get_block_size(),
            logical_block_start,
            buffer,
        )
    }

    fn get_inode(
        device: &mut D,
        superblock: &Superblock,
        block_group_descriptor_tables: &[BGD],
        inode_num: usize,
    ) -> Result<INode, Ext2Error> {
        let inode_size = superblock.s_inode_size as usize;
        let block_size = superblock.get_block_size();

        let block_group_number = (inode_num - 1) / superblock.s_inodes_per_group as usize;
        let inode_table_block =
            block_group_descriptor_tables[block_group_number].bg_inode_table as usize;

        let inode_table_index: usize = (inode_num - 1) % (superblock.s_inodes_per_group as usize);
        let inode_table_block_with_offset: usize =
            ((inode_table_index * inode_size) / block_size) + inode_table_block;
        let inode_table_interblock_offset: usize = (inode_table_index * inode_size) % block_size;

        let mut block_buffer: Vec<u8> = vec![0; block_size];

        for (i, chunk) in block_buffer.chunks_exact_mut(block_size).enumerate() {
            Self::read_logical_block_inner(
                device,
                block_size,
                inode_table_block_with_offset + i,
                chunk,
            )?;
        }

        let mut inode_data: [u8; size_of::<INode>()] = [0x00; size_of::<INode>()];

        inode_data
            .copy_from_slice(&block_buffer[inode_table_interblock_offset..][..size_of::<INode>()]);

        let inode: INode =
            unsafe { core::mem::transmute::<[u8; size_of::<INode>()], INode>(inode_data) };

        Ok(inode)
    }

    pub fn get_root_inode_wrapper(&mut self) -> Rc<INodeWrapper> {
        self.root_inode.clone()
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

        for (i, chunk) in descriptor_table_bytes_slice
            .chunks_exact_mut(superblock.get_block_size())
            .enumerate()
        {
            Self::read_logical_block_inner(
                &mut device,
                block_size,
                block_group_descriptor_block + i,
                chunk,
            )?;
        }

        block_group_descriptor_tables.truncate(descriptor_count);

        let root_inode: INode =
            Self::get_inode(&mut device, &superblock, &block_group_descriptor_tables, 2)?;

        Ok(Self {
            device,
            superblock,
            block_group_descriptor_tables,
            root_inode: Rc::new(INodeWrapper {
                inode: root_inode,
                _inode_num: 2,
            }),
        })
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
    ) -> Result<Rc<INodeWrapper>, Ext2Error> {
        if !node.is_dir() {
            return Err(Ext2Error::NotADirectory);
        }
        let inode_num = node.get_dir_entries(self, |dir_entry| {
            if dir_entry.name == name {
                ControlFlow::Break(dir_entry.inode_num)
            } else {
                ControlFlow::Continue(())
            }
        })?;
        let inode_num = inode_num.ok_or(Ext2Error::FileNotFound)?;

        // TODO: find out how to do operator overloading in rust and convert this into
        // TODO: a mut self method
        let inode: INode = Self::get_inode(
            &mut self.device,
            &self.superblock,
            &self.block_group_descriptor_tables,
            inode_num as usize,
        )?;

        Ok(Rc::new(INodeWrapper {
            inode,
            _inode_num: inode_num,
        }))
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

    fn get_word(byte_array: &[u8]) -> u32 {
        u32::from_le_bytes(*byte_array.first_chunk().unwrap())
    }

    pub fn get_inode_block_num<D: BlockDevice>(
        &self,
        number: usize,
        ext2: &mut Ext2<D>,
    ) -> Result<u32, Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();

        let block_inode_list_size: usize = block_size / size_of::<u32>();
        let block_inode_list_size_squared: usize = block_inode_list_size * block_inode_list_size;

        let logical_block_number;
        let mut block_buffer: Vec<u8> = vec![0; block_size];

        pub const TRIPLE_LINK_BLOCK_PTR_INDEX: usize = 14;
        pub const DOUBLE_LINK_BLOCK_PTR_INDEX: usize = 13;
        pub const SINGLE_LINK_BLOCK_PTR_INDEX: usize = 12;

        if number >= (12 + block_inode_list_size + block_inode_list_size_squared) {
            // hard mode: go through link to list of link of list of links to list of direct
            // block ptrs

            ext2.read_logical_block(
                self.inode.i_block[TRIPLE_LINK_BLOCK_PTR_INDEX] as usize,
                &mut block_buffer,
            )?;

            let second_level_base_num: usize =
                number - (12 + block_inode_list_size + block_inode_list_size_squared);
            let index: usize =
                (second_level_base_num / block_inode_list_size_squared) * size_of::<u32>();
            let block_second_level_index: u32 = Self::get_word(&block_buffer[index..index + 4]);

            ext2.read_logical_block(block_second_level_index as usize, &mut block_buffer)?;

            let first_level_base_num: usize = second_level_base_num % block_inode_list_size_squared;
            let block_buffer_second_index: usize =
                (first_level_base_num / block_inode_list_size) * size_of::<u32>();
            let block_first_level_index = Self::get_word(
                &block_buffer[block_buffer_second_index..block_buffer_second_index + 4],
            );

            ext2.read_logical_block(block_first_level_index as usize, &mut block_buffer)?;

            let block_buffer_first_index: usize =
                (first_level_base_num % block_inode_list_size) * size_of::<u32>();

            logical_block_number = Self::get_word(
                &block_buffer[block_buffer_first_index..block_buffer_first_index + 4],
            );
        } else if number >= 12 + block_inode_list_size {
            // medium: go through link to list of links to list of direct block ptrs
            ext2.read_logical_block(
                self.inode.i_block[DOUBLE_LINK_BLOCK_PTR_INDEX] as usize,
                &mut block_buffer,
            )?;

            let first_level_base_num: usize = number - (12 + block_inode_list_size);
            let index: usize = (first_level_base_num / block_inode_list_size) * size_of::<u32>();
            let block_first_level_index: usize =
                Self::get_word(&block_buffer[index..index + 4]) as usize;
            let block_final_level_index: usize =
                (first_level_base_num % block_inode_list_size) * size_of::<u32>();

            ext2.read_logical_block(block_first_level_index, &mut block_buffer)?;

            logical_block_number =
                Self::get_word(&block_buffer[block_final_level_index..block_final_level_index + 4]);
        } else if number >= 12 {
            // fairly easy: go through link to list of direct block ptrs
            ext2.read_logical_block(
                self.inode.i_block[SINGLE_LINK_BLOCK_PTR_INDEX] as usize,
                &mut block_buffer,
            )?;

            let index: usize = number - 12;
            let offset: usize = index * size_of::<u32>();

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
    ) -> Result<(), Ext2Error> {
        // TODO: caching
        let block_size = ext2.superblock.get_block_size();

        assert!(buffer.len() % block_size == 0);
        for (buf_segment, block_idx) in buffer
            .chunks_exact_mut(block_size)
            .zip(logical_block_start..)
        {
            let logical_block_num: usize = self.get_inode_block_num(block_idx, ext2)? as usize;
            ext2.read_logical_block(logical_block_num, buf_segment)?;
        }

        Ok(())
    }

    pub fn read_file<D: BlockDevice>(&self, ext2: &mut Ext2<D>) -> Result<Vec<u8>, Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let mut return_value: Vec<u8> = Vec::new();
        let blocks_to_read: usize = (self.size() as usize).div_ceil(block_size);
        return_value.resize(blocks_to_read * block_size, 0);

        self.read_block(0, return_value.as_mut_slice(), ext2)?;

        return_value.resize(self.size() as usize, 0);

        Ok(return_value)
    }

    pub fn read_text_file_as_str<D: BlockDevice>(
        &self,
        ext2: &mut Ext2<D>,
    ) -> Result<String, Ext2Error> {
        let bytes = self.read_file(ext2)?;
        String::from_utf8(bytes).map_err(|_| Ext2Error::NotUtf8)
    }

    pub fn get_dir_entries<D: BlockDevice, F, O>(
        &self,
        ext2: &mut Ext2<D>,
        mut callback: F,
    ) -> Result<Option<O>, Ext2Error>
    where
        F: FnMut(&DirEntry<'_>) -> ControlFlow<O>,
    {
        // TODO: caching
        let block_size: usize = ext2.superblock.get_block_size();
        let mut buffer = alloc::vec![0; block_size];

        let dir_size: usize = self.size() as usize;
        let dir_blocks: usize = dir_size.div_ceil(block_size);

        let mut i = 0;
        let mut block_idx = 0;

        while block_idx < dir_blocks {
            self.read_block(block_idx, buffer.as_mut_slice(), ext2)?;

            while i < block_size {
                // TODO: cleanly error on malformed directory entries
                let entry_start = &buffer[i..];
                assert!(entry_start.len() >= size_of::<DirEntryData>());
                let entry_data =
                    unsafe { entry_start.as_ptr().cast::<DirEntryData>().read_unaligned() };
                let name_start = &entry_start[size_of::<DirEntryData>()..];
                let name = &name_start[..entry_data.name_len as usize];

                let entry = DirEntry {
                    inode_num: entry_data.inode,
                    name_length: entry_data.name_len,
                    file_type: entry_data.file_type,
                    name,
                };

                match callback(&entry) {
                    ControlFlow::Continue(()) => (),
                    ControlFlow::Break(res) => return Ok(Some(res)),
                }

                i += entry_data.rec_len as usize;
            }

            let skip_blocks = i / block_size;
            block_idx += skip_blocks;
            i -= block_size * skip_blocks;
            assert!(skip_blocks == 1);
        }

        Ok(None)
    }
}
