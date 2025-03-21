use crate::bgd::bgd_misc_constants::INITIAL_BGD_SIZE;
use crate::bgd::BGD;
use crate::block_device::{BlockDevice, SECTOR_SIZE};
use crate::dir::{DirectoryEntryConstants, DirectoryEntryData, DirectoryEntryWrapper};
use crate::inode::i_mode::{EXT2_S_IFDIR, EXT2_S_IFREG};
use crate::inode::{
    ext4_xattr_entry_data, i_flags, i_mode, INode, INodeBlockInfo, INodeWrapper, REV_0_INODE_SIZE,
};
use crate::superblock::{s_feature_incompat, Superblock};
use crate::{get_epoch_time, BlockDeviceError, DeferredWriteMap, Ext2Error};
use alloc::rc::{Rc, Weak};
use bytemuck::bytes_of;
use std::cell::{Ref, RefCell};
use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::prelude::rust_2015::Vec;
use std::{print, vec};

pub struct Ext<Device> {
    pub(crate) device: Device,
    pub(crate) superblock: Superblock,
    pub(crate) block_group_descriptor_tables: Vec<BGD>,
    root_inode: Rc<RefCell<INodeWrapper>>,
    pub(crate) inode_map: BTreeMap<usize, Weak<RefCell<INodeWrapper>>>,
}

impl<D> Ext<D>
where
    D: BlockDevice,
{
    fn read_logical_block_inner(
        device: &mut D,
        superblock: &Superblock,
        logical_block_start: usize,
        buffer: &mut [u8],
        deferred_writes: Option<&DeferredWriteMap>,
    ) -> Result<(), Ext2Error> {
        let block_size: usize = superblock.get_block_size();
        assert_eq!(buffer.len(), block_size);
        let start_sector_numerator: usize = logical_block_start * block_size;
        let start_sector: usize = start_sector_numerator / SECTOR_SIZE;

        if deferred_writes.is_some() {
            let deferred_writes_unwrapped: &DeferredWriteMap = deferred_writes.unwrap();

            if deferred_writes_unwrapped.contains_key(&logical_block_start) {
                buffer[0..block_size]
                    .copy_from_slice(&deferred_writes_unwrapped[&logical_block_start]);

                return Ok(());
            }
        }

        let result = device.read_sectors(start_sector as u64, buffer);

        if result.is_err() {
            return Err(Ext2Error::BlockDeviceError(result.unwrap_err()));
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

    fn write_logical_block_inner(
        device: &mut D,
        superblock: &Superblock,
        logical_block_start: usize,
        buffer: &[u8],
    ) -> Result<(), Ext2Error> {
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

    pub fn write_logical_block(
        &mut self,
        logical_block_start: usize,
        buffer: &[u8],
    ) -> Result<(), Ext2Error> {
        Self::write_logical_block_inner(
            &mut self.device,
            &self.superblock,
            logical_block_start,
            buffer,
        )
    }

    fn read_logical_blocks_inner(
        device: &mut D,
        superblock: &Superblock,
        logical_block_start: usize,
        buffer: &mut [u8],
        deferred_writes: Option<&DeferredWriteMap>,
    ) -> Result<(), Ext2Error> {
        assert_eq!(buffer.len() % superblock.get_block_size(), 0);
        let logical_block_length = buffer.len() / superblock.get_block_size();

        for i in 0..logical_block_length {
            let slice_start: usize = i * superblock.get_block_size();
            let slice_end: usize = slice_start + superblock.get_block_size();

            Self::read_logical_block_inner(
                device,
                superblock,
                logical_block_start + i,
                &mut buffer[slice_start..slice_end],
                deferred_writes,
            )?
        }

        Ok(())
    }

    fn write_logical_blocks_inner(
        device: &mut D,
        superblock: &Superblock,
        logical_block_start: usize,
        buffer: &[u8],
    ) -> Result<(), Ext2Error> {
        assert_eq!(buffer.len() % superblock.get_block_size(), 0);
        let logical_block_length = buffer.len() / superblock.get_block_size();

        for i in 0..logical_block_length {
            let slice_start: usize = i * superblock.get_block_size();
            let slice_end: usize = slice_start + superblock.get_block_size();

            Self::write_logical_block_inner(
                device,
                superblock,
                logical_block_start + i,
                &buffer[slice_start..slice_end],
            )?
        }

        Ok(())
    }

    fn read_logical_blocks(
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

    fn write_logical_blocks(
        &mut self,
        logical_block_start: usize,
        buffer: &mut [u8],
    ) -> Result<(), Ext2Error> {
        Self::write_logical_block_inner(
            &mut self.device,
            &self.superblock,
            logical_block_start,
            buffer,
        )
    }

    pub(crate) fn get_block_that_has_inode(
        device: &mut D,
        superblock: &Superblock,
        block_group_descriptor_tables: &Vec<BGD>,
        inode_num: usize,
    ) -> INodeBlockInfo {
        //add flex_bg support!

        let inode_size = superblock.s_inode_size as usize;

        let block_group_number = (inode_num - 1) / superblock.s_inodes_per_group as usize;
        let inode_table_block =
            block_group_descriptor_tables[block_group_number].get_inode_table(superblock) as usize;

        let inode_table_index: usize = (inode_num - 1) % (superblock.s_inodes_per_group as usize);
        let inode_table_block_with_offset: usize =
            ((inode_table_index * inode_size) / superblock.get_block_size()) + inode_table_block;
        let inode_table_interblock_offset: usize =
            (inode_table_index * inode_size) % superblock.get_block_size();

        print!("");

        INodeBlockInfo {
            block_num: inode_table_block_with_offset,
            block_offset: inode_table_interblock_offset,
        }
    }

    fn get_inode_wrapper_inner(
        device: &mut D,
        superblock: &Superblock,
        block_group_descriptor_tables: &Vec<BGD>,
        inode_map: &mut BTreeMap<usize, Weak<RefCell<INodeWrapper>>>,
        inode_num: usize,
        deferred_writes: Option<&DeferredWriteMap>,
    ) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        let inode_block_info: INodeBlockInfo = Ext::get_block_that_has_inode(
            device,
            superblock,
            block_group_descriptor_tables,
            inode_num,
        );
        let mut block_buffer: Vec<u8> = vec![0; superblock.get_block_size()];

        Self::read_logical_block_inner(
            device,
            superblock,
            inode_block_info.block_num,
            block_buffer.as_mut_slice(),
            deferred_writes,
        )?;

        let mut inode = INode::new();
        let inode_bytes: &mut [u8] = bytemuck::bytes_of_mut(&mut inode);
        let inode_size: usize = if superblock.s_rev_level == 0 {
            REV_0_INODE_SIZE
        } else {
            std::cmp::min(size_of::<INode>(), superblock.s_inode_size as usize)
        };

        inode_bytes[0..inode_size].copy_from_slice(
            &block_buffer
                [inode_block_info.block_offset..inode_block_info.block_offset + inode_size],
        );

        let mut inode_xattrs_data: Vec<ext4_xattr_entry_data> = Vec::new();
        let mut inode_xattr_name_data: Vec<u8> = Vec::new();
        let inode_xattr_data_start: usize = inode_size + (4 - (inode_size % 4));
        let inode_xattr_data_end: usize = (inode.i_extra_isize as usize) + REV_0_INODE_SIZE;
        let mut i: usize = inode_xattr_data_start;

        while inode_xattr_data_end > i {
            let mut xattr = crate::inode::ext4_xattr_entry_data {
                e_name_len: 0,
                e_name_index: 0,
                e_value_offs: 0,
                e_value_inum: 0,
                e_value_size: 0,
                e_hash: 0,
                e_name: [],
            };
            let xattr_slice_end = i + size_of::<ext4_xattr_entry_data>();
            let xattr_name_end = xattr_slice_end + (xattr.e_name_len as usize);
            let mut xattr_bytes = bytemuck::bytes_of_mut(&mut xattr);

            xattr_bytes[0..].copy_from_slice(&block_buffer[i..xattr_slice_end]);

            inode_xattrs_data.push(xattr);

            inode_xattr_name_data.extend_from_slice(&block_buffer[xattr_slice_end..xattr_name_end]);

            i = xattr_name_end;
        }

        let inode_wrapper = Rc::new(RefCell::new(INodeWrapper {
            inode,
            _inode_num: inode_num as u32,
            inode_xattrs_data,
            inode_xattr_name_data,
        }));

        inode_map.insert(inode_num, Rc::downgrade(&inode_wrapper));

        Ok(inode_wrapper)
    }

    pub fn get_inode_wrapper(
        &mut self,
        inode_num: usize,
        deferred_writes: Option<&DeferredWriteMap>,
    ) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        Self::get_inode_wrapper_inner(
            &mut self.device,
            &self.superblock,
            &self.block_group_descriptor_tables,
            &mut self.inode_map,
            inode_num,
            deferred_writes,
        )
    }

    pub fn get_root_inode_wrapper(&mut self) -> Rc<RefCell<INodeWrapper>> {
        self.root_inode.clone()
    }

    pub fn add_block_group_deferred_write(
        &mut self,
        deferred_write_map: &mut DeferredWriteMap,
        block_group_num: usize,
    ) -> Result<(), Ext2Error> {
        let block_size: usize = self.superblock.get_block_size();
        let block_group_descriptor_block: usize = if block_size == 1024 { 2 } else { 1 }
            + ((block_group_num * size_of::<BGD>()) / block_size);
        let block_group_descriptor_offset: usize =
            (block_group_num * size_of::<BGD>()) % block_size;

        let mut block_group_copy: [u8; size_of::<BGD>()] = [0; size_of::<BGD>()];
        {
            let block_group_as_bytes =
                bytemuck::bytes_of(&self.block_group_descriptor_tables[block_group_num]);

            block_group_copy.copy_from_slice(block_group_as_bytes);
        }

        self.add_write_to_deferred_writes_map(
            deferred_write_map,
            block_group_descriptor_block,
            block_group_descriptor_offset,
            &block_group_copy,
            None,
        )?;

        Ok(())
    }

    pub fn add_super_block_deferred_write(
        &mut self,
        deferred_write_map: &mut DeferredWriteMap,
    ) -> Result<(), Ext2Error> {
        let mut superblock_bytes_copy: [u8; size_of::<Superblock>()] = [0; size_of::<Superblock>()];
        {
            let superblock_as_bytes = bytes_of(&self.superblock);

            superblock_bytes_copy.copy_from_slice(superblock_as_bytes);
        }

        // CHANGE BLOCK_NUM WHEN BLOCK_SIZE changes
        self.add_write_to_deferred_writes_map(
            deferred_write_map,
            1,
            0,
            &superblock_bytes_copy,
            None,
        )?;

        Ok(())
    }

    fn get_block_group_descriptor_tables(
        superblock: &Superblock,
        device: &mut D,
    ) -> Result<Vec<BGD>, Ext2Error> {
        let block_size: usize = superblock.get_block_size();
        let block_group_descriptor_block: usize = if block_size == 1024 { 2 } else { 1 };
        let supports_64bit =
            (superblock.s_feature_incompat & s_feature_incompat::EXT4_FEATURE_INCOMPAT_64BIT) == 0;
        let bgd_size: usize = if supports_64bit && superblock.s_desc_size > 32 {
            size_of::<BGD>()
        } else {
            INITIAL_BGD_SIZE
        };

        let descriptor_count = superblock.get_num_of_block_groups() as usize;
        let block_group_descriptor_blocks: usize =
            1 + (descriptor_count * bgd_size).div_ceil(block_size);
        let max_block_group_descriptors: usize =
            block_group_descriptor_blocks * (block_size / bgd_size);
        let block_group_descriptor_blocks_bgd_vec: usize =
            1 + (descriptor_count * bgd_size).div_ceil(size_of::<BGD>());

        let mut block_group_descriptor_tables: Vec<BGD> = Vec::new();

        block_group_descriptor_tables.resize(max_block_group_descriptors, BGD::new());

        let mut block_group_descriptor_tables_bytes: Vec<u8> =
            vec![0; max_block_group_descriptors * bgd_size];

        Self::read_logical_blocks_inner(
            device,
            &superblock,
            block_group_descriptor_block,
            block_group_descriptor_tables_bytes.as_mut_slice(),
            None,
        )?;

        let descriptor_table_bytes_ptr: *mut u8 =
            block_group_descriptor_tables.as_mut_ptr() as *mut u8;
        let bgd_vector_slice: &mut [u8] = unsafe {
            core::slice::from_raw_parts_mut(
                descriptor_table_bytes_ptr,
                block_group_descriptor_blocks_bgd_vec * block_size,
            )
        };
        let mut i: usize = 0;

        // we need to do this because bgd_size doesn't need to equal size_of::<BGD>()
        while max_block_group_descriptors > i {
            let table_bytes_slice_start = i * bgd_size;
            let table_bytes_slice_end = (i + 1) * bgd_size;

            let bgd_vec_slice_start = i * size_of::<BGD>();
            let bgd_vec_slice_end = (i + 1) * size_of::<BGD>();

            bgd_vector_slice[bgd_vec_slice_start..bgd_vec_slice_end].copy_from_slice(
                &block_group_descriptor_tables_bytes
                    [table_bytes_slice_start..table_bytes_slice_end],
            );

            i += 1;
        }

        Ok(block_group_descriptor_tables)
    }

    pub fn new(mut device: D) -> Result<Self, Ext2Error> {
        // TODO: avoid putting this buffer on the stack, and avoid storing
        // superblock padding in the Ext struct
        let mut buffer = [0; 1024];

        device.read_sectors(2, &mut buffer)?;

        let superblock: Superblock =
            unsafe { core::mem::transmute::<[u8; 1024], Superblock>(buffer) };

        let mut inode_map: BTreeMap<usize, Weak<RefCell<INodeWrapper>>> = BTreeMap::new();
        let block_group_descriptor_tables =
            Self::get_block_group_descriptor_tables(&superblock, &mut device)?;

        let root_inode_wrapper: Rc<RefCell<INodeWrapper>> = Self::get_inode_wrapper_inner(
            &mut device,
            &superblock,
            &block_group_descriptor_tables,
            &mut inode_map,
            2,
            None,
        )?;

        Ok(Self {
            device,
            superblock,
            block_group_descriptor_tables,
            root_inode: root_inode_wrapper,
            inode_map,
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
    ) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        if !node.is_dir() {
            return Err(Ext2Error::NotADirectory);
        }

        if (node.inode.i_flags & i_flags::EXT2_INDEX_FL) != 0 {
            node.find_dir_entry_hashed(self, name)
        } else {
            node.find_dir_entry_linear(self, name)
        }
    }

    pub fn find_recursive(
        &mut self,
        node: Rc<RefCell<INodeWrapper>>,
        name: &[u8],
        create_dirs_if_nonexistent: bool,
        create_file_if_nonexistent: bool,
    ) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        let path_split = name.split(|byte| *byte == b'/');
        let path_split_vec = path_split.collect::<Vec<&[u8]>>();
        let mut current_node: Rc<RefCell<INodeWrapper>> = node;
        let name_str = std::str::from_utf8(name);

        for (index, file_dir) in path_split_vec.iter().enumerate() {
            let file_dir_str = std::str::from_utf8(file_dir).unwrap();
            let mut current_node_result: Result<Rc<RefCell<INodeWrapper>>, Ext2Error> =
                self.find(&current_node.borrow(), file_dir);

            if current_node_result.is_err() {
                let ext2_error: Ext2Error = current_node_result.unwrap_err();
                let file_not_found: bool = ext2_error == Ext2Error::FileNotFound;

                if file_not_found && index != path_split_vec.len() - 1 && create_dirs_if_nonexistent
                {
                    let new_node = self.create_dir(&mut *current_node.borrow_mut(), *file_dir)?;

                    current_node = new_node;
                } else if file_not_found
                    && index == path_split_vec.len() - 1
                    && create_file_if_nonexistent
                {
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

    fn acquire_next_available_inode(
        &mut self,
        inode_data: INode,
        deferred_write_map: &mut DeferredWriteMap,
    ) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        let block_size: usize = self.superblock.get_block_size();
        let mut found_block_group_index_option: Option<usize> = None;

        for (block_group_index, mut block_group_table) in
            self.block_group_descriptor_tables.iter_mut().enumerate()
        {
            if block_group_table.get_free_inodes_count(&self.superblock) > 1 {
                let free_inodes_count: u64 =
                    block_group_table.get_free_inodes_count(&self.superblock);

                block_group_table.set_free_inodes_count(free_inodes_count - 1, &self.superblock);

                self.superblock.s_free_inodes_count -= 1;
                found_block_group_index_option = Some(block_group_index);
                break;
            }
        }

        if found_block_group_index_option.is_some() {
            let mut block_buffer = vec![0; block_size];
            let found_block_group_index: usize = found_block_group_index_option.unwrap();
            let inode_bitmap_num = self.block_group_descriptor_tables[found_block_group_index]
                .get_inode_bitmap(&self.superblock) as usize;
            let mut byte_write: [u8; 1] = [0; 1];
            let mut byte_write_pos: usize = 0;

            self.read_logical_block(
                inode_bitmap_num,
                &mut block_buffer,
                Some(deferred_write_map),
            )?;

            let mut found_new_inode: bool = false;
            let new_inode_num_base: usize =
                (self.superblock.s_inodes_per_group as usize) * found_block_group_index;
            let mut new_inode_num: usize = 0;
            let mut inode_byte_offset = 0;

            let first_non_reserved_inode: usize = if self.superblock.s_rev_level == 1 {
                self.superblock.s_first_ino as usize
            } else {
                11
            };

            for (inode_bitmap_byte_index, inode_bitmap_byte) in block_buffer.iter().enumerate() {
                for i in 0..8 {
                    let current_relative_inode_num = (inode_bitmap_byte_index * 8) + i;
                    let inode_reserved = current_relative_inode_num >= first_non_reserved_inode
                        && found_block_group_index == 0;

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

            self.add_write_to_deferred_writes_map(
                deferred_write_map,
                inode_bitmap_num,
                byte_write_pos,
                &byte_write,
                Some(block_buffer.clone()),
            )?;

            new_inode_num += 1;

            let num_of_inodes_per_block: usize = block_size / size_of::<INode>();
            let inode_block_index: usize =
                (self.block_group_descriptor_tables[found_block_group_index]
                    .get_inode_table(&self.superblock) as usize)
                    + ((new_inode_num - 1) / num_of_inodes_per_block);
            let inode_block_offset: usize =
                ((new_inode_num - 1) % num_of_inodes_per_block) * size_of::<INode>();

            self.read_logical_block(
                inode_block_index,
                block_buffer.as_mut_slice(),
                Some(deferred_write_map),
            )?;

            let inode_bytes = bytemuck::bytes_of(&inode_data);

            block_buffer[inode_block_offset..inode_block_offset + size_of::<INode>()]
                .copy_from_slice(inode_bytes);

            self.add_write_to_deferred_writes_map(
                deferred_write_map,
                inode_block_index,
                inode_block_offset,
                inode_bytes,
                None,
            )?;

            new_inode_num += new_inode_num_base;

            self.add_block_group_deferred_write(
                deferred_write_map,
                found_block_group_index_option.unwrap(),
            )?;
            self.add_super_block_deferred_write(deferred_write_map)?;

            return Ok(Rc::new(RefCell::new(INodeWrapper {
                inode: inode_data,
                _inode_num: new_inode_num as u32,
                inode_xattrs_data: vec![],
                inode_xattr_name_data: vec![],
            })));
        }

        Err(Ext2Error::UnavailableINode)
    }

    pub fn add_write_to_deferred_writes_map(
        &mut self,
        deferred_write_map: &mut DeferredWriteMap,
        block_num: usize,
        start_write: usize,
        write_bytes: &[u8],
        optional_block_buffer: Option<Vec<u8>>,
    ) -> Result<(), Ext2Error> {
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
                self.read_logical_block(
                    block_num,
                    block_buffer.as_mut_slice(),
                    Some(deferred_write_map),
                )?;
            }

            deferred_write_map.insert(block_num, block_buffer);
        }

        let block_buffer = deferred_write_map.get_mut(&block_num).unwrap();

        block_buffer[start_write..start_write + write_bytes.len()].copy_from_slice(write_bytes);

        Ok(())
    }

    pub fn write_back_deferred_writes(
        &mut self,
        mut deferred_writes: DeferredWriteMap,
    ) -> Result<(), Ext2Error> {
        self.superblock.s_wtime = get_epoch_time() as u32;
        self.add_super_block_deferred_write(&mut deferred_writes)?;

        for mut deferred_write in deferred_writes {
            self.write_logical_block(deferred_write.0, deferred_write.1.as_mut_slice())?;
        }

        Ok(())
    }

    pub fn create_file_with_mode(
        &mut self,
        node: &mut INodeWrapper,
        name: &[u8],
        i_mode: u16,
        deferred_writes: &mut DeferredWriteMap,
    ) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        // what do we need to do when creating a new file?
        // go thru BGD inode bitmaps, find the next unallocated inode number and update it
        // update inode number
        // add a directory entry pointing to our inode thru append_file
        if !node.is_dir() {
            return Err(Ext2Error::InvalidMode);
        }

        let epoch_time: usize = get_epoch_time();

        let new_inode = INode::new();

        if name.len() > DirectoryEntryConstants::MAX_FILE_NAME_LEN {
            return Err(Ext2Error::TooLongFileName);
        }

        let new_inode_wrapper = self.acquire_next_available_inode(new_inode, deferred_writes)?;

        let dir_entry_name_length: u16 =
            std::cmp::min(name.len(), DirectoryEntryConstants::MAX_FILE_NAME_LEN) as u16;
        let mut new_dir_entry_wrapper = DirectoryEntryWrapper {
            entry: DirectoryEntryData {
                inode_num: new_inode_wrapper.borrow()._inode_num,
                rec_len: (DirectoryEntryConstants::MIN_DIRECTORY_ENTRY_SIZE as u16)
                    + dir_entry_name_length,
                name_len: dir_entry_name_length as u8,
                file_type: 0,
                name_characters: [0; 256],
            },
            inode_block_num: 0,
            offset: 0,
        };

        let name_string = std::str::from_utf8(name).unwrap();

        new_dir_entry_wrapper.entry.name_characters
            [0..(new_dir_entry_wrapper.entry.name_len as usize)]
            .copy_from_slice(name);

        let mut current_inter_block_offset: usize = 0;

        // TODO(Bobby): deal with case where there is enough block space for
        // TODO(Bobby): dir entry but not padding
        if new_dir_entry_wrapper.entry.rec_len % 4 != 0 {
            new_dir_entry_wrapper.entry.rec_len += 4 - (new_dir_entry_wrapper.entry.rec_len % 4);
        }
        let mut found_empty_dir_entry: bool = false;

        let empty_dir_entry_wrapper: Option<DirectoryEntryWrapper> = node.get_dir_entries(
            self,
            |dir_entry_wrapper| {
                let mut mutable_entry_for_dir_entry_wrapper = dir_entry_wrapper.entry.clone();
                let mut prior_dir_entry_allocated_size: usize =
                    DirectoryEntryConstants::MIN_DIRECTORY_ENTRY_SIZE
                        + (dir_entry_wrapper.entry.name_len as usize);
                let mut dir_entry_padding: usize = 0;

                if (prior_dir_entry_allocated_size % 4) != 0 {
                    dir_entry_padding = 4 - (prior_dir_entry_allocated_size % 4);
                }

                current_inter_block_offset += dir_entry_wrapper.entry.rec_len as usize;

                if dir_entry_wrapper.entry.rec_len as usize
                    >= prior_dir_entry_allocated_size
                        + dir_entry_padding
                        + new_dir_entry_wrapper.entry.rec_len as usize
                {
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

                    ControlFlow::Break(DirectoryEntryWrapper {
                        entry: mutable_entry_for_dir_entry_wrapper,
                        inode_block_num: dir_entry_wrapper.inode_block_num,
                        offset: dir_entry_wrapper.offset,
                    })
                } else {
                    ControlFlow::Continue(())
                }
            },
            None,
        )?;

        if empty_dir_entry_wrapper.is_some() {
            let mut dir_entry_wrapper: DirectoryEntryWrapper = empty_dir_entry_wrapper.unwrap();

            dir_entry_wrapper.add_deferred_write(self, node, deferred_writes)?;
            new_dir_entry_wrapper.add_deferred_write(self, node, deferred_writes)?;
        } else {
            // we will need to allocate a new block of dir entries
            let block_size: usize = self.superblock.get_block_size();

            assert_eq!((node.size() as usize) % block_size, 0);
            assert!(
                (block_size - current_inter_block_offset)
                    < new_dir_entry_wrapper.entry.rec_len as usize
            );

            let mut new_dir_entry: DirectoryEntryData = new_dir_entry_wrapper.entry;
            let mut dir_entry_bytes_with_padding = vec![0; block_size];

            let actual_rec_len: usize = new_dir_entry.rec_len as usize;

            new_dir_entry.rec_len = block_size as u16;

            let dir_entry_bytes = bytemuck::bytes_of(&new_dir_entry);
            let remaining_bytes_in_block: usize = block_size - current_inter_block_offset;

            dir_entry_bytes_with_padding
                [remaining_bytes_in_block..remaining_bytes_in_block + actual_rec_len]
                .copy_from_slice(&dir_entry_bytes[0..actual_rec_len]);

            node.append_file_no_writeback(
                self,
                dir_entry_bytes_with_padding.as_slice(),
                true,
                deferred_writes,
            )?;
        }

        new_inode_wrapper
            .borrow_mut()
            .get_deferred_write_inode(self, deferred_writes)?;

        self.inode_map.insert(
            new_inode_wrapper.borrow()._inode_num as usize,
            Rc::downgrade(&new_inode_wrapper),
        );

        Ok(new_inode_wrapper)
    }

    // EXT2_S_IROTH and EXT2_S_IXOTH is needed for fuse tests to succeed
    // without sudo escalation
    pub fn create_dir(
        &mut self,
        node: &mut INodeWrapper,
        name: &[u8],
    ) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        let block_size: usize = self.superblock.get_block_size();
        let mut deferred_writes: DeferredWriteMap = BTreeMap::new();
        let dir_node: Rc<RefCell<INodeWrapper>> = self.create_file_with_mode(
            node,
            name,
            EXT2_S_IFDIR | i_mode::EXT2_S_IROTH | i_mode::EXT2_S_IXOTH,
            &mut deferred_writes,
        )?;

        let mut cur_dir_entry = DirectoryEntryWrapper {
            entry: DirectoryEntryData {
                inode_num: dir_node.borrow()._inode_num,
                rec_len: (DirectoryEntryConstants::MIN_DIRECTORY_ENTRY_SIZE as u16) + 1,
                name_len: 1,
                file_type: 0,
                name_characters: [0; 256],
            },
            inode_block_num: 0,
            offset: 0,
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

        dir_node.borrow_mut().append_file_no_writeback(
            self,
            &dir_entry_bytes,
            true,
            &mut deferred_writes,
        )?;

        dir_node
            .borrow_mut()
            .get_deferred_write_inode(self, &mut deferred_writes)?;

        let writeback_result = self.write_back_deferred_writes(deferred_writes);

        if writeback_result.is_err() {
            Err(writeback_result.unwrap_err())
        } else {
            Ok(dir_node)
        }
    }

    // Creates a file named name (<= 255 characters)
    // EXT2_S_IROTH is needed for fuse tests to succeed
    pub fn create_file(
        &mut self,
        node: &mut INodeWrapper,
        name: &[u8],
    ) -> Result<Rc<RefCell<INodeWrapper>>, Ext2Error> {
        let mut deferred_writes: DeferredWriteMap = BTreeMap::new();

        let file_node = self.create_file_with_mode(
            node,
            name,
            EXT2_S_IFREG | i_mode::EXT2_S_IROTH,
            &mut deferred_writes,
        )?;
        let writeback_result = self.write_back_deferred_writes(deferred_writes);

        if writeback_result.is_err() {
            Err(writeback_result.unwrap_err())
        } else {
            Ok(file_node)
        }
    }

    pub fn num_of_block_groups(&self) -> usize {
        let num_of_block_groups_from_blocks: usize = ((self.superblock.s_blocks_count as f32)
            / (self.superblock.s_blocks_per_group as f32))
            .ceil() as usize;
        let num_of_block_groups_from_inodes: usize = ((self.superblock.s_inodes_count as f32)
            / (self.superblock.s_inodes_per_group as f32))
            .ceil() as usize;

        assert_eq!(
            num_of_block_groups_from_blocks,
            num_of_block_groups_from_inodes
        );

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
