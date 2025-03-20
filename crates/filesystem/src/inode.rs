use crate::bgd::BGD;
use crate::block_device::BlockDevice;
use crate::dir::{
    dirhash, DirectoryEntryData, DirectoryEntryWrapper, HTreeDirectoryEntry,
    HTreeDirectoryEntryNode, HTreeDirectoryEntryRoot,
};
use crate::ext::Ext;
use crate::Ext2Error::{FileNotFound, NotEnoughDeviceSpace};
use crate::{hash, DeferredWriteMap, Ext2Error, UNALLOCATED_BLOCK_SLOT};
use alloc::rc::Rc;
use std::cell::{Ref, RefCell};
use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::prelude::rust_2015::Vec;
use std::{print, slice, vec};
use bytemuck::{Pod, Zeroable};
use crate::inode::i_flags::{EXT4_EXTENTS_FL, EXT4_INLINE_DATA_FL};

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

    pub const EXT4_NOTAIL_FL: u32 = 0x00008000;
    pub const EXT4_DIRSYNC_FL: u32 = 0x00010000;
    pub const EXT4_TOPDIR_FL: u32 = 0x00020000;
    pub const EXT4_HUGE_FILE_FL: u32 = 0x00040000;
    pub const EXT4_EXTENTS_FL: u32 = 0x00080000;
    pub const EXT4_EA_INODE_FL: u32 = 0x00200000;
    pub const EXT4_EOFBLOCKS_FL: u32 = 0x00400000;
    pub const EXT4_SNAPFILE_FL: u32 = 0x01000000;
    pub const EXT4_SNAPFILE_DELETED_FL: u32 = 0x04000000;
    pub const EXT4_SNAPFILE_SHRUNK_FL: u32 = 0x08000000;
    pub const EXT4_INLINE_DATA_FL: u32 = 0x10000000;
    pub const EXT4_PROJINHERIT_FL: u32 = 0x20000000;
    pub const EXT4_RESERVED_FL: u32 = 0x80000000;
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ext4_extent_header {
    eh_magic: u16,
    eh_entries: u16,
    eh_max: u16,
    eh_depth: u16,
    eh_generation: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ext4_extent_idx {
    ei_block: u32,
    ei_leaf_lo: u32,
    ei_leaf_hi: u16,
    ei_unused: u16,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ext4_extent {
    ee_block: u32,
    ee_len: u16,
    ee_start_hi: u16,
    ee_start_lo: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ext4_extent_tail {
    eb_checksum: u32,
}

pub(crate) struct INodeBlockInfo {
    pub(crate) block_num: usize,
    pub(crate) block_offset: usize,
}

pub const REV_0_INODE_SIZE: usize = 128;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
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
    pub(crate) i_flags: u32,
    i_osd1: u32,
    i_block: [u32; 15], // 12 direct, single, double, triple.
    // even in 64-bit mode i_block are 32-bit. HOWEVER,
    // "Files not using extents (i.e. files using block maps) must be placed within the first
    // 2^32 blocks of a filesystem. Files with extents must be placed within the first 2^48
    // blocks of a filesystem. It’s not clear what happens with larger filesystems."
    i_generation: u32,
    i_file_acl: u32,
    i_size_high: u32,
    i_obso_faddr: u32, // obsolete
    i_osd2: [u8; 12],

    // end of original 128-sized inodes. everything below only applies to ext2 revision 1 and up
    pub(crate) i_extra_isize: u16,
    i_checksum_hi: u16,
    i_ctime_extra: u32,
    i_mtime_extra: u32,
    i_atime_extra: u32,
    i_crtime: u32,
    i_crtime_extra: u32,
    i_version_hi: u32,
    i_projid: u32,
}

impl INode {
    pub(crate) fn new() -> INode {
        INode {
            i_mode: 0,
            i_uid: 0,
            i_size: 0,
            i_atime: 0,
            i_ctime: 0,
            i_mtime: 0,
            i_dtime: 0,
            i_gid: 0,
            i_links_count: 0,
            i_blocks: 0,
            i_flags: 0,
            i_osd1: 0,
            i_block: [0; 15],
            i_generation: 0,
            i_file_acl: 0,
            i_size_high: 0,
            i_obso_faddr: 0,
            i_osd2: [0; 12],
            i_extra_isize: 0,
            i_checksum_hi: 0,
            i_ctime_extra: 0,
            i_mtime_extra: 0,
            i_atime_extra: 0,
            i_crtime: 0,
            i_crtime_extra: 0,
            i_version_hi: 0,
            i_projid: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ext4_xattr_header {
    h_magic: u32,
    h_refcount: u32,
    h_blocks: u32,
    h_hash: u32,
    h_checksum: u32,
    h_reserved: [u32; 2]
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct ext4_xattr_entry_data {
    pub(crate) e_name_len: u8,
    pub(crate) e_name_index: u8,
    pub(crate) e_value_offs: u16,
    pub(crate) e_value_inum: u32,
    pub(crate) e_value_size: u32,
    pub(crate) e_hash: u32,
    pub(crate) e_name: [u8; 0]
}

struct ext4_xattr_entry<'a> {
    e_name_len: u8,
    e_name_index: u8,
    e_value_offs: u16,
    e_value_inum: u32,
    e_value_size: u32,
    e_hash: u32,
    e_name: &'a [u8]
}

#[repr(C)]
#[derive(Debug)]
pub struct INodeWrapper {
    pub(crate) inode: INode,
    pub(crate) _inode_num: u32,

    pub(crate) inode_xattrs_data: Vec<ext4_xattr_entry_data>,
    pub(crate) inode_xattr_name_data: Vec<u8>
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
        (self.inode.i_size as u64) | ((self.inode.i_size_high as u64) << 32)
    }

    pub fn update_size<D: BlockDevice>(&mut self, new_size: u64, ext2: &Ext<D>) {
        self.inode.i_size = ((new_size << 32) >> 32) as u32;
        self.inode.i_size_high = (new_size >> 32) as u32;
    }

    pub fn get_deferred_write_inode<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        deferred_write_map: &mut DeferredWriteMap,
    ) -> Result<(), Ext2Error> {
        let inode_block_info: INodeBlockInfo = Ext::get_block_that_has_inode(
            &mut ext2.device,
            &ext2.superblock,
            &ext2.block_group_descriptor_tables,
            self._inode_num as usize,
        );
        let inode_bytes = bytemuck::bytes_of(&self.inode);
        let inode_byte_slice: &[u8] = if ext2.superblock.s_rev_level == 0 {
            &inode_bytes[0..REV_0_INODE_SIZE]
        } else {
            &inode_bytes[0..ext2.superblock.s_inode_size as usize]
        };

        ext2.add_write_to_deferred_writes_map(
            deferred_write_map,
            inode_block_info.block_num,
            inode_block_info.block_offset,
            inode_byte_slice,
            None,
        )?;

        let x_attr_start = ext2.superblock.s_inode_size as usize;
        let x_attr_byte_size = (self.inode.i_extra_isize as usize) - x_attr_start;
        let mut i: usize = 0;
        let mut x_attr_index: usize = 0;
        let mut name_index: usize = 0;
        let mut x_attr_buffer: Vec<u8> = Vec::new();

        x_attr_buffer.resize(x_attr_byte_size, 0);

        // extended attributes have to start at aa 4-byte alignment
        i += (4 - (ext2.superblock.s_inode_size % 4)) as usize;

        while x_attr_byte_size > i {
            let current_x_attr: &ext4_xattr_entry_data = &self.inode_xattrs_data[x_attr_index];
            let current_x_attr_bytes: &[u8] =
                bytemuck::bytes_of(current_x_attr);

            x_attr_buffer[i..(i + current_x_attr_bytes.len())].copy_from_slice(current_x_attr_bytes);

            i += size_of::<ext4_xattr_entry>();

            let x_attr_slice_end = i + (current_x_attr.e_name_len as usize);
            let string_slice_end = name_index + (current_x_attr.e_name_len as usize);
            
            x_attr_buffer[i..x_attr_slice_end].copy_from_slice(
                &self.inode_xattr_name_data[name_index..string_slice_end]);

            x_attr_index += 1;
            name_index += current_x_attr.e_name_len as usize;
            i += current_x_attr.e_name_len as usize;
        }

        ext2.add_write_to_deferred_writes_map(
            deferred_write_map,
            inode_block_info.block_num,
            inode_block_info.block_offset + x_attr_start,
            x_attr_buffer.as_slice(),
            None,
        )?;

        Ok(())
    }

    pub fn get_block_group_index<D: BlockDevice>(&self, ext2: &Ext<D>) -> usize {
        (self._inode_num / ext2.superblock.s_inodes_per_group) as usize
    }

    pub fn block_allocated_count<D: BlockDevice>(&self, ext2: &Ext<D>) -> usize {
        (self.inode.i_blocks / (2 << ext2.superblock.s_log_block_size)) as usize
    }

    pub fn set_block_allocated_count<D: BlockDevice>(&mut self, ext2: &Ext<D>, blocks: usize) {
        self.inode.i_blocks = (blocks as u32) * (2 << ext2.superblock.s_log_block_size);
    }

    fn get_word(byte_array: &[u8]) -> u32 {
        u32::from_le_bytes(*byte_array.first_chunk().unwrap())
    }

    pub const TRIPLE_LINK_BLOCK_PTR_INDEX: usize = 14;
    pub const DOUBLE_LINK_BLOCK_PTR_INDEX: usize = 13;
    pub const SINGLE_LINK_BLOCK_PTR_INDEX: usize = 12;

    pub fn get_inode_block_num_addressing_mode<D: BlockDevice>(
        &self,
        number: usize,
        ext2: &mut Ext<D>,
        deferred_writes: Option<&DeferredWriteMap>
    ) -> Result<u64, Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let block_inode_list_size: usize = block_size / size_of::<u32>();
        let block_inode_list_size_squared: usize = block_inode_list_size * block_inode_list_size;

        let mut logical_block_number: u32 = 0;
        let mut block_buffer = vec![0; block_size];

        if number
            >= (Self::SINGLE_LINK_BLOCK_PTR_INDEX
            + block_inode_list_size
            + block_inode_list_size_squared)
        {
            // hard mode: go through link to list of link of list of links to list of direct
            // block ptrs

            ext2.read_logical_block(
                self.inode.i_block[Self::TRIPLE_LINK_BLOCK_PTR_INDEX] as usize,
                &mut block_buffer,
                deferred_writes,
            )?;

            let second_level_base_num: usize =
                number - (12 + block_inode_list_size + block_inode_list_size_squared);
            let index: usize =
                (second_level_base_num / block_inode_list_size_squared) * size_of::<u32>();
            let block_second_level_index: u32 = Self::get_word(&block_buffer[index..index + 4]);

            ext2.read_logical_block(
                block_second_level_index as usize,
                &mut block_buffer,
                deferred_writes,
            )?;

            let first_level_base_num: usize = second_level_base_num % block_inode_list_size_squared;
            let block_buffer_second_index: usize =
                (first_level_base_num / block_inode_list_size) * size_of::<u32>();
            let block_first_level_index = Self::get_word(
                &block_buffer[block_buffer_second_index..block_buffer_second_index + 4],
            );

            ext2.read_logical_block(
                block_first_level_index as usize,
                &mut block_buffer,
                deferred_writes,
            )?;

            let block_buffer_first_index: usize =
                (first_level_base_num % block_inode_list_size) * size_of::<u32>();

            logical_block_number = Self::get_word(
                &block_buffer[block_buffer_first_index..block_buffer_first_index + 4],
            );
        } else if number >= Self::SINGLE_LINK_BLOCK_PTR_INDEX + block_inode_list_size {
            // medium: go through link to list of links to list of direct block ptrs
            ext2.read_logical_block(
                self.inode.i_block[Self::DOUBLE_LINK_BLOCK_PTR_INDEX] as usize,
                &mut block_buffer,
                deferred_writes,
            )?;

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
            ext2.read_logical_block(
                self.inode.i_block[Self::SINGLE_LINK_BLOCK_PTR_INDEX] as usize,
                &mut block_buffer,
                deferred_writes,
            )?;

            let index: usize = number - 12;
            let offset: usize = index * size_of::<u32>();

            let block_buffer_u32_slice: &[u32] = bytemuck::cast_slice::<_, u32>(&mut block_buffer);

            logical_block_number = Self::get_word(&block_buffer[offset..offset + 4]);
        } else {
            // easy: go through direct block ptrs
            logical_block_number = self.inode.i_block[number];
        }

        Ok(logical_block_number as u64)
    }

    pub fn get_inode_block_extents<D: BlockDevice>(
        &self,
        number: usize,
        ext2: &mut Ext<D>,
        deferred_writes: Option<&DeferredWriteMap>
    ) -> Result<u64, Ext2Error> {
        const EXT4_EXTENT_HEADER_MAGIC: u16 = 0xF30A;
        const EXT4_EXTENT_UNINIT: u16 = 32768;

        assert_eq!(size_of::<ext4_extent_header>() % size_of::<u32>(), 0);

        let header_blocks_slice_end: usize = size_of::<ext4_extent_header>() / size_of::<u32>();

        let mut current_extent_header_slice: &[u8] =
            bytemuck::cast_slice(&self.inode.i_block[0..header_blocks_slice_end]);
        let mut block_to_read: Option<usize> = None;
        let mut found_data_block: bool = false;
        let mut block_buffer: Vec<u8> = vec![0u8; ext2.superblock.get_block_size()];

        while !found_data_block {
            let mut current_entry_index: usize = 0;
            let mut current_extent_header =
                unsafe { current_extent_header_slice.align_to::<ext4_extent_header>().1[0] };
            let is_leaf_node = current_extent_header.eh_depth == 0;

            while (current_extent_header.eh_entries as usize) > current_entry_index {
                if current_extent_header.eh_magic != EXT4_EXTENT_HEADER_MAGIC {
                    return Err(Ext2Error::InvalidExtentTree);
                }

                if is_leaf_node {
                    let current_extent_leaf_node_slice: &[u8] =
                        &current_extent_header_slice[header_blocks_slice_end..(header_blocks_slice_end + size_of::<ext4_extent>())];
                    let current_extent_leaf_node =
                        unsafe { current_extent_leaf_node_slice.align_to::<ext4_extent>().1[0] };
                    let leaf_block = current_extent_leaf_node.ee_block as usize;
                    
                    if number > leaf_block {
                        let leaf_node_length = 
                            if current_extent_leaf_node.ee_len > EXT4_EXTENT_UNINIT {
                                (current_extent_leaf_node.ee_len - EXT4_EXTENT_UNINIT) as usize
                            } else {
                                current_extent_leaf_node.ee_len as usize
                            };

                        if leaf_block + leaf_node_length > number {
                            block_to_read = Some(((current_extent_leaf_node.ee_start_hi as usize) << 32) |
                                            current_extent_leaf_node.ee_start_lo as usize);
                            found_data_block = true;
                            break;
                        }
                    }
                } else {
                    let current_extent_interior_node_slice: &[u8] =
                        &current_extent_header_slice[header_blocks_slice_end..(header_blocks_slice_end + size_of::<ext4_extent_idx>())];
                    let current_extent_interior_node =
                        unsafe { current_extent_interior_node_slice.align_to::<ext4_extent_idx>().1[0] };

                    // we assume that the entries of interior nodes are in increasing order
                    // otherwise this doesn't work
                    if number > (current_extent_interior_node.ei_block as usize) {
                        block_to_read =
                            Some(((current_extent_interior_node.ei_leaf_hi as usize) << 32) |
                                 (current_extent_interior_node.ei_leaf_lo as usize));
                    } else {
                        break;
                    }
                }

                current_entry_index += 1;
            }

            if block_to_read.is_some() {
                ext2.read_logical_block(block_to_read.unwrap(), &mut block_buffer, deferred_writes)?;
                current_extent_header_slice = &block_buffer[0..header_blocks_slice_end];
            } else {
                // we were unable to find a block extent, which means this block doesn't exist
                // in extents
                return Err(Ext2Error::ExtentNotFound);
            }
        }

        Ok(block_to_read.unwrap() as u64)
    }

    pub fn get_inode_block_num<D: BlockDevice>(
        &self,
        number: usize,
        ext2: &mut Ext<D>,
        deferred_writes: Option<&DeferredWriteMap>,
    ) -> Result<u64, Ext2Error> {
        if (self.inode.i_flags & EXT4_EXTENTS_FL) != 0 {
            self.get_inode_block_extents(number, ext2, deferred_writes)
        } else {
            self.get_inode_block_num_addressing_mode(number, ext2, deferred_writes)
        }
    }

    pub fn read_block<D: BlockDevice>(
        &self,
        logical_block_start: usize,
        buffer: &mut [u8],
        ext2: &mut Ext<D>,
        deferred_writes: Option<&DeferredWriteMap>,
    ) -> Result<(), Ext2Error> {
        // TODO: caching
        let block_size = ext2.superblock.get_block_size();
        let file_size: usize = self.size() as usize;
        assert_eq!(buffer.len() % block_size, 0);

        if (self.inode.i_flags & EXT4_INLINE_DATA_FL) != 0 && file_size <= 60 {
            // ext4's inline data feature in i_blocks
            // TODO(Bobby): implement inline data in ext attributes

            for i in 0..(file_size / size_of::<u32>()) {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        self.inode.i_block[i..(i + 1)].as_ptr().cast(),
                        buffer[(i * size_of::<u32>())..((i + 1) * size_of::<u32>())].as_mut_ptr(),
                        size_of::<u32>(),
                    )
                }
            }
        } else {
            for (buf_segment, block_idx) in buffer
                .chunks_exact_mut(block_size)
                .zip(logical_block_start..)
            {
                let logical_block_num: usize =
                    self.get_inode_block_num(block_idx, ext2, deferred_writes)? as usize;
                ext2.read_logical_block(logical_block_num, buf_segment, deferred_writes)?;
            }
        }

        Ok(())
    }

    pub fn read_file<D: BlockDevice>(&self, ext2: &mut Ext<D>) -> Result<Vec<u8>, Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let mut return_value: Vec<u8> = Vec::new();
        let blocks_to_read: usize = (self.size() as usize).div_ceil(block_size);
        return_value.resize(blocks_to_read * block_size, 0);

        self.read_block(0, return_value.as_mut_slice(), ext2, None)?;

        return_value.resize(self.size() as usize, 0);

        Ok(return_value)
    }

    pub fn get_dir_entries_inner<D: BlockDevice, F, O>(
        &self,
        ext2: &mut Ext<D>,
        callback: &mut F,
        block_num: usize,
        dir_entries_data: &[u8],
        inner_block_counter: &mut usize,
        deferred_writes: Option<&DeferredWriteMap>,
    ) -> Result<Option<O>, Ext2Error>
    where
        F: FnMut(DirectoryEntryWrapper) -> ControlFlow<O>,
    {
        while *inner_block_counter < dir_entries_data.len() {
            // TODO: cleanly error on malformed directory entries
            let entry_start = &dir_entries_data[*inner_block_counter..];
            //assert!(entry_start.len() >= size_of::<DirectoryEntryData>());
            let entry_data = unsafe {
                entry_start
                    .as_ptr()
                    .cast::<DirectoryEntryData>()
                    .read_unaligned()
            };
            let name_start = &entry_start[(size_of::<DirectoryEntryData>() - 256)..];
            let name = &name_start[..entry_data.name_len as usize];

            let mut entry = DirectoryEntryWrapper {
                entry: DirectoryEntryData {
                    inode_num: entry_data.inode_num,
                    rec_len: entry_data.rec_len,
                    name_len: entry_data.name_len,
                    file_type: entry_data.file_type,
                    name_characters: [0; 256],
                },
                inode_block_num: block_num,
                offset: *inner_block_counter,
            };

            assert!(entry_data.rec_len > 8);

            entry.entry.name_characters[..entry_data.name_len as usize].copy_from_slice(name);

            match callback(entry) {
                ControlFlow::Continue(()) => (),
                ControlFlow::Break(res) => return Ok(Some(res)),
            }

            *inner_block_counter += entry_data.rec_len as usize;
        }

        Ok(None)
    }

    pub fn get_dir_entries<D: BlockDevice, F, O>(
        &self,
        ext2: &mut Ext<D>,
        mut callback: F,
        deferred_writes: Option<&DeferredWriteMap>,
    ) -> Result<Option<O>, Ext2Error>
    where
        F: FnMut(DirectoryEntryWrapper) -> ControlFlow<O>,
    {
        let block_size: usize = ext2.superblock.get_block_size();
        let mut buffer = alloc::vec![0; block_size];

        let dir_size: usize = self.size() as usize;
        let dir_blocks: usize = dir_size.div_ceil(block_size);

        let mut inner_block_offset = 0;
        let mut block_idx = 0;

        while block_idx < dir_blocks {
            self.read_block(block_idx, buffer.as_mut_slice(), ext2, deferred_writes)?;

            let dir_entries_option: Option<O> = self.get_dir_entries_inner(
                ext2,
                &mut callback,
                block_idx,
                &buffer[inner_block_offset..],
                &mut inner_block_offset,
                deferred_writes,
            )?;

            if dir_entries_option.is_some() {
                return Ok(dir_entries_option);
            }

            let skip_blocks = inner_block_offset / block_size;
            block_idx += skip_blocks;
            inner_block_offset -= block_size * skip_blocks;
            assert_eq!(skip_blocks, 1);
        }

        Ok(None)
    }

    pub(crate) fn find_dir_entry_linear<D: BlockDevice>(
        &self,
        ext2: &mut Ext<D>,
        name: &[u8],
    ) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        let name_str: &str = std::str::from_utf8(name).unwrap();
        let inode_num: Option<u32> = self.get_dir_entries(
            ext2,
            |dir_entry| {
                let name_str = std::str::from_utf8(name);
                let current_name_str = std::str::from_utf8(
                    &dir_entry.entry.name_characters[0..dir_entry.entry.name_len as usize],
                );

                if current_name_str == name_str {
                    ControlFlow::Break(dir_entry.entry.inode_num)
                } else {
                    ControlFlow::Continue(())
                }
            },
            None,
        )?;
        let inode_num: u32 = inode_num.ok_or(Ext2Error::FileNotFound)?;

        let return_value: Rc<RefCell<INodeWrapper>> = ext2.get_inode_wrapper(inode_num as usize, None)?;

        ext2.inode_map
            .insert(inode_num as usize, Rc::downgrade(&return_value));

        Ok(return_value)
    }

    pub(crate) fn find_dir_entry_hashed<D: BlockDevice>(
        &self,
        ext2: &mut Ext<D>,
        name: &[u8],
    ) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let mut block_buffer = alloc::vec![0; block_size];

        self.read_block(0, block_buffer.as_mut_slice(), ext2, None)?;

        let mut filename_hash: u32 = 0;

        let mut current_tree_level: usize;
        let mut current_node_length: usize;
        let mut target_htree_entry: HTreeDirectoryEntry;
        let mut indirect_levels: usize;

        {
            let dir_entry_root: &HTreeDirectoryEntryRoot = unsafe {
                &block_buffer
                    .as_slice()
                    .align_to::<HTreeDirectoryEntryRoot>()
                    .1[0]
            };

            let filename_hash_result: Result<u32, Ext2Error> =
                self.filename_dir_hash(name, dir_entry_root.hash_version);

            if filename_hash_result.is_err() {
                return Err(filename_hash_result.unwrap_err());
            }

            filename_hash = filename_hash_result.unwrap();

            current_tree_level = 0;
            current_node_length = dir_entry_root.count as usize;
            target_htree_entry = HTreeDirectoryEntry {
                hash: 0,
                block: dir_entry_root.block,
            };
            indirect_levels = dir_entry_root.indirect_levels as usize;
        }

        while current_tree_level < indirect_levels {
            let htree_entry_start: usize = if current_tree_level == 0 { 0x28 } else { 0x12 };
            let (_, mut htree_slice, _) = unsafe {
                block_buffer.as_mut_slice()[htree_entry_start..].align_to::<HTreeDirectoryEntry>()
            };

            let mut start: usize = 0;
            let mut end: usize = current_node_length - 1;

            while start <= end {
                let index: usize = (start + end) / 2;
                let current_tree_entry: HTreeDirectoryEntry = htree_slice[index];

                if index != (current_node_length - 1) && filename_hash > htree_slice[index + 1].hash
                {
                    start = index + 1;
                } else if htree_slice[index].hash > filename_hash {
                    end = index - 1;
                } else {
                    target_htree_entry = current_tree_entry;
                    break;
                }
            }

            current_tree_level += 1;

            self.read_block(
                target_htree_entry.block as usize,
                block_buffer.as_mut_slice(),
                ext2,
                None,
            )?;

            if current_tree_level < indirect_levels {
                // the inode number must be zero to appear like this entry isn't in use
                assert_eq!(Self::get_word(&block_buffer[0..4]), 0);

                let htree_dir_entry_node: &HTreeDirectoryEntryNode = unsafe {
                    &block_buffer
                        .as_slice()
                        .align_to::<HTreeDirectoryEntryNode>()
                        .1[0]
                };

                current_node_length = htree_dir_entry_node.count as usize;
                target_htree_entry.block = htree_dir_entry_node.block;
            }
        }

        let mut inner_block_offset: usize = 0;
        let inode_num: Option<u32> = self.get_dir_entries_inner(
            ext2,
            &mut |dir_entry| {
                let name_str = std::str::from_utf8(name);
                let current_name_str = std::str::from_utf8(
                    &dir_entry.entry.name_characters[0..dir_entry.entry.name_len as usize],
                );

                if current_name_str == name_str {
                    ControlFlow::Break(dir_entry.entry.inode_num)
                } else {
                    ControlFlow::Continue(())
                }
            },
            target_htree_entry.block as usize,
            block_buffer.as_slice(),
            &mut inner_block_offset,
            None,
        )?;

        if inode_num.is_none() {
            // TODO: Check if hash collision flag is set
            return Err(FileNotFound);
        }

        let inode_num: u32 = inode_num.ok_or(Ext2Error::FileNotFound)?;

        let return_value: Rc<RefCell<INodeWrapper>> = 
            ext2.get_inode_wrapper(inode_num as usize, None)?;

        ext2.inode_map
            .insert(inode_num as usize, Rc::downgrade(&return_value));

        Ok(return_value)
    }

    pub fn find_new_blocks<D: BlockDevice>(
        &self,
        ext2: &mut Ext<D>,
        num_of_blocks: usize,
        all_blocks_or_fail: bool,
        deferred_writes: &mut DeferredWriteMap,
    ) -> Result<Vec<usize>, Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let num_of_block_groups: usize = ext2.num_of_block_groups();
        let num_of_blocks_per_block_group: usize = ext2.superblock.s_blocks_per_group as usize;
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
                current_block_group_index = (current_block_group_index + 1) % num_of_block_groups;
                current_block_group =
                    &ext2.block_group_descriptor_tables[current_block_group_index];

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

                ext2.read_logical_block(
                    current_block_bitmap_block,
                    block_buffer.as_mut_slice(),
                    Some(deferred_writes),
                )?;

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
                    assert!(
                        blocks_allocated_from_block_group == needed_blocks
                            || blocks_allocated_from_block_group == free_block_count
                    );

                    ext2.block_group_descriptor_tables[current_block_group_index]
                        .bg_free_blocks_count -= blocks_allocated_from_block_group as u16;
                    ext2.superblock.s_free_blocks_count -= blocks_allocated_from_block_group as u32;

                    ext2.add_super_block_deferred_write(deferred_writes)?;
                    ext2.add_block_group_deferred_write(
                        deferred_writes,
                        current_block_group_index,
                    )?;

                    for byte_write in byte_writes {
                        ext2.add_write_to_deferred_writes_map(
                            deferred_writes,
                            current_block_bitmap_block,
                            byte_write.0,
                            slice::from_ref(&byte_write.1),
                            Some(block_buffer.clone()),
                        )?;
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

    fn write_indirected_block_to_inode<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        block_num: usize,
        blocks_allocated_within_list: usize,
        num_of_blocks_allocated: &mut usize,
        blocks_newly_allocated: &mut usize,
        new_blocks: &[usize],
        deferred_write_map: &mut DeferredWriteMap,
    ) -> Result<(), Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let mut block_buffer = vec![0; block_size];
        let singly_indirect_block_block_limit: usize = std::cmp::min(
            block_size / size_of::<u32>(),
            new_blocks.len() - *blocks_newly_allocated,
        );

        ext2.read_logical_block(
            block_num,
            block_buffer.as_mut_slice(),
            Some(deferred_write_map),
        )?;

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
        ext2.add_write_to_deferred_writes_map(
            deferred_write_map,
            block_num,
            0,
            &block_buffer,
            Some(block_buffer.clone()),
        )?;

        Ok(())
    }

    fn allocate_blocks_for_block_list<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        start_of_list: usize,
        end_of_list: usize,
        block_list_buffer: &mut [u8],
        new_block_storage_blocks_allocated: &mut usize,
        all_blocks_or_fail: bool,
        deferred_writes: &mut DeferredWriteMap,
    ) -> Result<Vec<usize>, Ext2Error> {
        let mut new_blocks_needed_for_block_list_count: usize = 0;

        ext2.read_logical_block(
            self.inode.i_block[Self::DOUBLE_LINK_BLOCK_PTR_INDEX] as usize,
            block_list_buffer,
            Some(deferred_writes),
        )?;

        let block_list_u32_slice: &[u32] = bytemuck::cast_slice::<u8, u32>(block_list_buffer);

        for block_num in start_of_list..end_of_list {
            if block_list_u32_slice[block_num] == UNALLOCATED_BLOCK_SLOT {
                new_blocks_needed_for_block_list_count += 1;
            }
        }

        let result: Result<Vec<usize>, Ext2Error> = self.find_new_blocks(
            ext2,
            new_blocks_needed_for_block_list_count,
            all_blocks_or_fail,
            deferred_writes,
        );
        if result.is_ok() {
            let result_val = result.unwrap();

            *new_block_storage_blocks_allocated += result_val.len();

            Ok(result_val)
        } else {
            result
        }
    }

    fn write_doublely_indirect_blocks_to_inode<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        double_indirect_block_num: usize,
        num_of_blocks_allocated: &mut usize,
        all_blocks_or_fail: bool,
        blocks_newly_allocated: &mut usize,
        new_block_storage_blocks_allocated: &mut usize,
        new_blocks: &[usize],
        deferred_writes: &mut DeferredWriteMap,
    ) -> Result<(), Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let count_of_doubly_linked_blocks: usize = *num_of_blocks_allocated
            - (ext2.get_inline_block_capacity() + ext2.get_single_indirect_block_capacity());

        let starting_doubly_linked_block: usize =
            count_of_doubly_linked_blocks / ext2.get_single_indirect_block_capacity();
        let ending_doubly_linked_block: usize = std::cmp::min(
            ext2.get_single_indirect_block_capacity(),
            new_blocks.len()
                - blocks_newly_allocated.div_ceil(ext2.get_single_indirect_block_capacity()),
        );
        let mut block_list_buffer = vec![0; block_size];

        let new_block_list_blocks = self.allocate_blocks_for_block_list(
            ext2,
            starting_doubly_linked_block,
            ending_doubly_linked_block,
            &mut block_list_buffer,
            new_block_storage_blocks_allocated,
            all_blocks_or_fail,
            deferred_writes,
        )?;
        let mut new_block_list_index: usize = 0;

        for block_num in starting_doubly_linked_block..ending_doubly_linked_block {
            let num_of_blocks_allocated_within_list: usize =
                if block_num == starting_doubly_linked_block {
                    (*num_of_blocks_allocated - ext2.get_inline_block_capacity())
                        % ext2.get_single_indirect_block_capacity()
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

                ext2.add_write_to_deferred_writes_map(
                    deferred_writes,
                    double_indirect_block_num,
                    block_num * size_of::<u32>(),
                    &block_list_buffer
                        [block_num * size_of::<u32>()..(block_num + 1) * size_of::<u32>()],
                    None,
                )?;
            }

            current_block_slot =
                bytemuck::cast_slice::<u8, u32>(block_list_buffer.as_mut_slice())[block_num];

            self.write_indirected_block_to_inode(
                ext2,
                current_block_slot as usize,
                num_of_blocks_allocated_within_list,
                num_of_blocks_allocated,
                blocks_newly_allocated,
                new_blocks,
                deferred_writes,
            )?;
        }

        Ok(())
    }

    fn write_triplely_indirect_blocks_to_inode<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        num_of_blocks_allocated: &mut usize,
        all_blocks_or_fail: bool,
        blocks_newly_allocated: &mut usize,
        new_blocks: &[usize],
        new_block_storage_blocks_allocated: &mut usize,
        deferred_writes: &mut DeferredWriteMap,
    ) -> Result<(), Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();

        self.allocate_indirect_list_block_if_needed(
            ext2,
            Self::TRIPLE_LINK_BLOCK_PTR_INDEX,
            new_block_storage_blocks_allocated,
            all_blocks_or_fail,
            deferred_writes,
        )?;

        let mut block_list_buffer = vec![0; block_size];

        ext2.read_logical_block(
            self.inode.i_block[Self::TRIPLE_LINK_BLOCK_PTR_INDEX] as usize,
            block_list_buffer.as_mut_slice(),
            Some(deferred_writes),
        )?;

        let starting_triply_directed_list_block: usize = (*num_of_blocks_allocated
            - ext2.get_double_indirect_block_capacity())
            / ext2.get_double_indirect_block_capacity();
        let ending_triply_directed_list_block: usize = std::cmp::min(
            ext2.get_triple_indirect_block_capacity(),
            new_blocks.len()
                - blocks_newly_allocated.div_ceil(ext2.get_double_indirect_block_capacity()),
        );
        let mut block_list_buffer = vec![0; block_size];

        let new_block_list_blocks: Vec<usize> = self.allocate_blocks_for_block_list(
            ext2,
            starting_triply_directed_list_block,
            ending_triply_directed_list_block,
            &mut block_list_buffer,
            new_block_storage_blocks_allocated,
            all_blocks_or_fail,
            deferred_writes,
        )?;
        let mut new_block_list_index: usize = 0;
        let triply_indirect_block_list: &mut [u32] =
            bytemuck::cast_slice_mut::<u8, u32>(block_list_buffer.as_mut_slice());

        for i in starting_triply_directed_list_block..ending_triply_directed_list_block {
            if triply_indirect_block_list[i] == UNALLOCATED_BLOCK_SLOT {
                triply_indirect_block_list[i] = new_block_list_blocks[new_block_list_index] as u32;

                new_block_list_index += 1;
            }

            self.write_doublely_indirect_blocks_to_inode(
                ext2,
                triply_indirect_block_list[i] as usize,
                num_of_blocks_allocated,
                all_blocks_or_fail,
                blocks_newly_allocated,
                new_block_storage_blocks_allocated,
                new_blocks,
                deferred_writes,
            )?;
        }

        Ok(())
    }

    fn allocate_indirect_list_block_if_needed<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        indirect_index: usize,
        new_block_storage_blocks_allocated: &mut usize,
        all_blocks_or_fail: bool,
        deferred_writes: &mut DeferredWriteMap,
    ) -> Result<(), Ext2Error> {
        if self.inode.i_block[indirect_index] == UNALLOCATED_BLOCK_SLOT {
            let new_blocks_allocated: Vec<usize> =
                self.find_new_blocks::<D>(ext2, 1, all_blocks_or_fail, deferred_writes)?;

            if new_blocks_allocated.is_empty() {
                return Err(NotEnoughDeviceSpace);
            }

            self.inode.i_block[indirect_index] = new_blocks_allocated[0] as u32;

            *new_block_storage_blocks_allocated += 1;
        }
        Ok(())
    }

    fn write_new_blocks_to_inode<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        new_blocks: &[usize],
        new_block_storage_blocks_allocated: &mut usize,
        all_blocks_or_fail: bool,
        deferred_writes: &mut DeferredWriteMap,
    ) -> Result<usize, Ext2Error> {
        // assumption: references to block zero means unallocated block slot
        // TODO: write inode to disk
        // TODO: write new block num to inode
        // need to handle unallocated blocks containing doubly linked inode blocks
        // and singly linked inode block
        let block_size: usize = ext2.superblock.get_block_size();
        let mut num_of_blocks_allocated: usize = (self.size() as usize).div_ceil(block_size);

        let mut blocks_newly_allocated: usize = 0;

        if num_of_blocks_allocated < ext2.get_inline_block_capacity()
            && blocks_newly_allocated < new_blocks.len()
        {
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

        if num_of_blocks_allocated < single_block_limit && blocks_newly_allocated < new_blocks.len()
        {
            let single_indirect_new_blocks_slice: &[usize] = new_blocks;
            let num_of_blocks_allocated_within_list: usize =
                num_of_blocks_allocated - ext2.get_inline_block_capacity();

            self.allocate_indirect_list_block_if_needed(
                ext2,
                Self::SINGLE_LINK_BLOCK_PTR_INDEX,
                new_block_storage_blocks_allocated,
                all_blocks_or_fail,
                deferred_writes,
            )?;

            self.write_indirected_block_to_inode(
                ext2,
                self.inode.i_block[Self::SINGLE_LINK_BLOCK_PTR_INDEX] as usize,
                num_of_blocks_allocated_within_list,
                &mut num_of_blocks_allocated,
                &mut blocks_newly_allocated,
                single_indirect_new_blocks_slice,
                deferred_writes,
            )?;
        }

        if blocks_newly_allocated < new_blocks.len() && num_of_blocks_allocated < double_block_limit
        {
            // find all blocks needed to complete the write
            self.allocate_indirect_list_block_if_needed(
                ext2,
                Self::DOUBLE_LINK_BLOCK_PTR_INDEX,
                new_block_storage_blocks_allocated,
                all_blocks_or_fail,
                deferred_writes,
            )?;

            self.write_doublely_indirect_blocks_to_inode(
                ext2,
                self.inode.i_block[Self::DOUBLE_LINK_BLOCK_PTR_INDEX] as usize,
                &mut num_of_blocks_allocated,
                all_blocks_or_fail,
                &mut blocks_newly_allocated,
                new_block_storage_blocks_allocated,
                new_blocks,
                deferred_writes,
            )?;
        }

        if blocks_newly_allocated < new_blocks.len() {
            self.write_triplely_indirect_blocks_to_inode(
                ext2,
                &mut num_of_blocks_allocated,
                all_blocks_or_fail,
                &mut blocks_newly_allocated,
                new_blocks,
                new_block_storage_blocks_allocated,
                deferred_writes,
            )?;
        }

        Ok(blocks_newly_allocated)
    }

    pub(crate) fn append_file_no_writeback<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        new_data: &[u8],
        all_bytes_or_fail: bool,
        deferred_writes: &mut DeferredWriteMap,
    ) -> Result<usize, Ext2Error> {
        let block_size: usize = ext2.superblock.get_block_size();
        let allocated_block_count: usize = (self.size() as usize).div_ceil(block_size);
        let base_allocated_block: Option<usize> = if allocated_block_count == 0 {
            None
        } else {
            Some(
                self.get_inode_block_num(allocated_block_count - 1, ext2, Some(deferred_writes))?
                    as usize,
            )
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
        if base_allocated_block_offset == 0
            || (block_size - base_allocated_block_offset) < new_data.len()
        {
            let new_blocks_allocated_result = self.find_new_blocks::<D>(
                ext2,
                image_file_size_in_blocks,
                all_bytes_or_fail,
                deferred_writes,
            );

            let old_new_blocks_allocated_size: usize = new_blocks_allocated.len();

            new_blocks_allocated.append(&mut new_blocks_allocated_result?);

            let new_blocks_slice: &[usize] = &new_blocks_allocated[old_new_blocks_allocated_size..];

            self.write_new_blocks_to_inode(
                ext2,
                new_blocks_slice,
                &mut new_block_storage_blocks_allocated,
                true,
                deferred_writes,
            )?;
        }

        let new_data_block_allocated_num: usize =
            allocated_block_count + new_blocks_allocated.len();

        for new_block in new_blocks_allocated {
            let write_base =
                if base_allocated_block.is_some() && new_block == base_allocated_block.unwrap() {
                    base_allocated_block_offset
                } else {
                    0
                };
            let write_size = std::cmp::min(
                if base_allocated_block.is_some() && new_block == base_allocated_block.unwrap() {
                    block_size - base_allocated_block_offset
                } else {
                    block_size
                },
                new_data.len() - bytes_written,
            );

            let current_byte_slice: &[u8] = &new_data[bytes_written..bytes_written + write_size];

            bytes_written += write_size;

            ext2.add_write_to_deferred_writes_map(
                deferred_writes,
                new_block,
                write_base,
                current_byte_slice,
                None,
            )?;
        }

        self.update_size(self.size() + (bytes_written as u64), ext2);
        self.set_block_allocated_count(
            ext2,
            new_data_block_allocated_num + new_block_storage_blocks_allocated,
        );
        self.get_deferred_write_inode(ext2, deferred_writes)?;

        Ok(bytes_written)
    }

    // append to file, with the new file size being the existing file size + size of new_data
    pub fn append_file<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        new_data: &[u8],
        all_bytes_or_fail: bool,
    ) -> Result<usize, Ext2Error> {
        let mut deferred_writes: DeferredWriteMap = BTreeMap::new();
        let bytes_written: usize =
            self.append_file_no_writeback(ext2, new_data, all_bytes_or_fail, &mut deferred_writes)?;

        ext2.write_back_deferred_writes(deferred_writes)?;

        Ok(bytes_written)
    }

    pub fn truncate_file<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        num_bytes: u64,
    ) -> Result<u64, Ext2Error> {
        // TODO:
        let mut deferred_writes = BTreeMap::new();
        let block_size: usize = ext2.superblock.get_block_size();
        let block_info: INodeBlockInfo = Ext::get_block_that_has_inode(
            &mut ext2.device,
            &ext2.superblock,
            &ext2.block_group_descriptor_tables,
            self._inode_num as usize,
        );
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
                let logical_block_num =
                    self.get_inode_block_num(i, ext2, Some(&deferred_writes))?;
                // update block bitmap
                let block_group_num = logical_block_num / (ext2.superblock.s_blocks_per_group as u64);
                let index = logical_block_num % (ext2.superblock.s_blocks_per_group as u64);

                let mut block_buffer = vec![0; block_size];

                ext2.read_logical_block(
                    ext2.block_group_descriptor_tables[block_group_num as usize].bg_block_bitmap
                        as usize,
                    block_buffer.as_mut_slice(),
                    Some(&deferred_writes),
                )?;
                let mut block_buffer_byte = block_buffer[index as usize / 8];
                block_buffer_byte &= 0b11111111 - (1 << (index % 8));
                ext2.add_write_to_deferred_writes_map(
                    &mut deferred_writes,
                    ext2.block_group_descriptor_tables[block_group_num as usize].bg_block_bitmap
                        as usize,
                    index as usize,
                    slice::from_ref(&block_buffer_byte),
                    Some(block_buffer),
                )?;

                // increment free blocks count
                ext2.block_group_descriptor_tables[block_group_num as usize]
                    .bg_free_blocks_count += 1;
                ext2.superblock.s_free_blocks_count += 1;

                ext2.add_block_group_deferred_write(
                    &mut deferred_writes,
                    block_group_num as usize,
                )?;
            }
            ext2.add_super_block_deferred_write(&mut deferred_writes)?;

            self.update_size(self.size() - num_bytes_remaining_removed as u64, ext2);
            self.get_deferred_write_inode(ext2, &mut deferred_writes)?;
        }
        ext2.write_back_deferred_writes(deferred_writes)?;
        Ok(num_bytes)
    }

    // overwrite over file, with the new file size being the size of new_data
    pub fn overwrite_file<D: BlockDevice>(
        &mut self,
        ext2: &mut Ext<D>,
        new_data: &[u8],
        all_bytes_or_fail: bool,
    ) -> Result<usize, Ext2Error> {
        // TODO(Sasha): Handle partial writes, properly report bytes written
        let block_size: usize = ext2.superblock.get_block_size();
        let allocated_block_count = (self.size() as usize).div_ceil(block_size);
        let mut deferred_writes: DeferredWriteMap = BTreeMap::new();
        let mut bytes_written: u64 = 0;
        if allocated_block_count == new_data.len().div_ceil(block_size) {
            // easy case
            for i in 0..allocated_block_count {
                let cur_block = self.get_inode_block_num(i, ext2, Some(&deferred_writes))? as usize;
                let current_byte_slice: &[u8] =
                    &new_data[i * block_size..std::cmp::min((i + 1) * block_size, new_data.len())];
                ext2.add_write_to_deferred_writes_map(
                    &mut deferred_writes,
                    cur_block,
                    0,
                    current_byte_slice,
                    None,
                )?;
                bytes_written +=
                    (std::cmp::min((i + 1) * block_size, new_data.len()) - i * block_size) as u64;
            }
            self.update_size(bytes_written, ext2);
            self.get_deferred_write_inode(ext2, &mut deferred_writes)?;
            ext2.write_back_deferred_writes(deferred_writes)?;
        } else if allocated_block_count < new_data.len().div_ceil(block_size) {
            // allocate more
            for i in 0..allocated_block_count {
                let cur_block = self.get_inode_block_num(i, ext2, Some(&deferred_writes))? as usize;
                let current_byte_slice: &[u8] = &new_data[i * block_size..(i + 1) * block_size];
                ext2.add_write_to_deferred_writes_map(
                    &mut deferred_writes,
                    cur_block,
                    0,
                    current_byte_slice,
                    None,
                )?;
                bytes_written += ((i + 1) * block_size - i * block_size) as u64;
            }
            self.update_size(bytes_written, ext2);
            self.get_deferred_write_inode(ext2, &mut deferred_writes)?;
            ext2.write_back_deferred_writes(deferred_writes)?;
            let new_slice: &[u8] = &new_data[allocated_block_count * block_size..new_data.len()];
            bytes_written += self.append_file(ext2, new_slice, all_bytes_or_fail)? as u64;
        } else {
            self.truncate_file(ext2, new_data.len() as u64 - self.size() as u64);
            for i in 0..allocated_block_count {
                let cur_block = self.get_inode_block_num(i, ext2, Some(&deferred_writes))? as usize;
                let current_byte_slice: &[u8] =
                    &new_data[i * block_size..std::cmp::min((i + 1) * block_size, new_data.len())];
                ext2.add_write_to_deferred_writes_map(
                    &mut deferred_writes,
                    cur_block,
                    0,
                    current_byte_slice,
                    None,
                )?;
                bytes_written +=
                    (std::cmp::min((i + 1) * block_size, new_data.len()) - i * block_size) as u64;
            }
            self.update_size(bytes_written, ext2);
            self.get_deferred_write_inode(ext2, &mut deferred_writes)?;
            ext2.write_back_deferred_writes(deferred_writes)?;
        }
        Ok(new_data.len())
    }

    pub fn delete_file<D: BlockDevice>(&mut self, ext2: &mut Ext<D>) -> Result<usize, Ext2Error> {
        let block_size = ext2.superblock.get_block_size();
        let size = self.size() as usize;
        self.truncate_file(ext2, self.size());
        let actual_inode_num = self._inode_num - 1; // inodes are 1 indexed
        let block_group_num =
            actual_inode_num as usize / ext2.superblock.s_blocks_per_group as usize;
        let index = actual_inode_num as usize % ext2.superblock.s_blocks_per_group as usize;
        let mut block_buffer = vec![0; block_size];

        let mut deferred_writes: DeferredWriteMap = BTreeMap::new();

        ext2.read_logical_block(
            ext2.block_group_descriptor_tables[block_group_num as usize].bg_inode_bitmap as usize,
            block_buffer.as_mut_slice(),
            Some(&deferred_writes),
        )?;
        let mut block_buffer_byte = block_buffer[index as usize / 8];
        block_buffer_byte &= 0b11111111 - (1 << (index % 8));
        ext2.add_write_to_deferred_writes_map(
            &mut deferred_writes,
            ext2.block_group_descriptor_tables[block_group_num as usize].bg_inode_bitmap as usize,
            index as usize,
            slice::from_ref(&block_buffer_byte),
            Some(block_buffer),
        )?;

        ext2.block_group_descriptor_tables[block_group_num].bg_free_inodes_count -= 1;
        ext2.superblock.s_free_inodes_count += 1;
        //TODO: if this is the last inode in this block, should we deallocate the block?
        ext2.add_block_group_deferred_write(&mut deferred_writes, block_group_num as usize)?;
        ext2.add_super_block_deferred_write(&mut deferred_writes)?;
        //TODO: probably want to delete myself
        Ok(size)
    }

    pub fn filename_dir_hash(&self, name: &[u8], hash_version: u8) -> Result<u32, Ext2Error> {
        match hash_version {
            dirhash::LEGACY | dirhash::LEGACY_UNSIGNED => {
                Ok(hash::hash_legacy(name, hash_version == dirhash::LEGACY))
            }
            dirhash::HALF_MD4 | dirhash::HALF_MD4_UNSIGNED => Err(Ext2Error::NotYetImplemented),
            dirhash::TEA | dirhash::TEA_UNSIGNED => Err(Ext2Error::NotYetImplemented),
            _ => Err(Ext2Error::UnsupportedDirHashVersion),
        }
    }
}
