use crate::block_device::BlockDevice;
use crate::ext::Ext;
use crate::superblock::{s_feature_incompat, Superblock};

pub mod bgd_misc_constants {
    pub const INITIAL_BGD_SIZE: usize = 32;
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BGD {
    bg_block_bitmap_lo: u32,
    bg_inode_bitmap_lo: u32,
    bg_inode_table_lo: u32,
    bg_free_blocks_count_lo: u16,
    bg_free_inodes_count_lo: u16,
    bg_used_dirs_count_lo: u16,
    pub(crate) bg_flags: u16,
    bg_exclude_bitmap_lo: u32,
    bg_block_bitmap_csum_lo: u16,
    bg_inode_bitmap_csum_lo: u16,
    bg_itable_unused_lo: u16,
    pub(crate) bg_checksum: u16,
    // From ext4 docs: "These fields only exist if the 64bit feature is enabled and s_desc_size > 32."
    bg_block_bitmap_hi: u32,
    bg_inode_bitmap_hi: u32,
    bg_inode_table_hi: u32,
    bg_free_blocks_count_hi: u16,
    bg_free_inodes_count_hi: u16,
    bg_used_dirs_count_hi: u16,
    bg_itable_unused_hi: u16,
    bg_exclude_bitmap_hi: u32,
    bg_block_bitmap_csum_hi: u16,
    bg_inode_bitmap_csum_hi: u16,
    pub(crate) bg_reserved: u32,
}

impl BGD {
    fn use_64bit(superblock: &Superblock) -> bool {
        (superblock.s_feature_incompat & s_feature_incompat::EXT4_FEATURE_INCOMPAT_64BIT) == 0
            && superblock.s_desc_size > 32
    }

    fn get_64bit_extended_val(lo: u32, hi: u32, superblock: &Superblock) -> u64 {
        (lo as u64)
            | (if Self::use_64bit(superblock) {
                (hi as u64) << 32
            } else {
                0x0
            })
    }

    fn set_64bit_extended_val(val: u64, lo: &mut u32, hi: &mut u32, superblock: &Superblock) {
        if Self::use_64bit(superblock) {
            *hi = (val >> 32) as u32;
        }

        *lo = val as u32;
    }

    fn set_64bit_extended_val_u16(val: u64, lo: &mut u16, hi: &mut u16, superblock: &Superblock) {
        if Self::use_64bit(superblock) {
            *hi = (val >> 16) as u16;
        }

        *lo = val as u16;
    }

    pub fn new() -> BGD {
        BGD {
            bg_block_bitmap_lo: 0,
            bg_inode_bitmap_lo: 0,
            bg_inode_table_lo: 0,
            bg_free_blocks_count_lo: 0,
            bg_free_inodes_count_lo: 0,
            bg_used_dirs_count_lo: 0,
            bg_flags: 0,
            bg_exclude_bitmap_lo: 0,
            bg_block_bitmap_csum_lo: 0,
            bg_inode_bitmap_csum_lo: 0,
            bg_itable_unused_lo: 0,
            bg_checksum: 0,
            bg_block_bitmap_hi: 0,
            bg_inode_bitmap_hi: 0,
            bg_inode_table_hi: 0,
            bg_free_blocks_count_hi: 0,
            bg_free_inodes_count_hi: 0,
            bg_used_dirs_count_hi: 0,
            bg_itable_unused_hi: 0,
            bg_exclude_bitmap_hi: 0,
            bg_block_bitmap_csum_hi: 0,
            bg_inode_bitmap_csum_hi: 0,
            bg_reserved: 0,
        }
    }

    pub fn get_block_bitmap(&self, superblock: &Superblock) -> u64 {
        Self::get_64bit_extended_val(self.bg_block_bitmap_lo, self.bg_block_bitmap_hi, superblock)
    }

    pub fn get_inode_bitmap(&self, superblock: &Superblock) -> u64 {
        Self::get_64bit_extended_val(self.bg_inode_bitmap_lo, self.bg_inode_bitmap_hi, superblock)
    }

    pub fn get_inode_table(&self, superblock: &Superblock) -> u64 {
        Self::get_64bit_extended_val(self.bg_inode_table_lo, self.bg_inode_table_hi, superblock)
    }

    pub fn get_free_block_count(&self, superblock: &Superblock) -> u64 {
        Self::get_64bit_extended_val(
            self.bg_free_blocks_count_lo as u32,
            self.bg_free_blocks_count_hi as u32,
            superblock,
        )
    }

    pub fn get_free_inodes_count(&self, superblock: &Superblock) -> u64 {
        Self::get_64bit_extended_val(
            self.bg_free_inodes_count_lo as u32,
            self.bg_free_inodes_count_hi as u32,
            superblock,
        )
    }

    pub fn get_used_dir_count(&self, superblock: &Superblock) -> u64 {
        Self::get_64bit_extended_val(
            self.bg_used_dirs_count_lo as u32,
            self.bg_used_dirs_count_hi as u32,
            superblock,
        )
    }

    pub fn get_exclude_bitmap(&self, superblock: &Superblock) -> u64 {
        Self::get_64bit_extended_val(
            self.bg_exclude_bitmap_lo,
            self.bg_exclude_bitmap_hi,
            superblock,
        )
    }

    pub fn get_block_bitmap_csum(&self, superblock: &Superblock) -> u64 {
        Self::get_64bit_extended_val(
            self.bg_block_bitmap_csum_lo as u32,
            self.bg_block_bitmap_csum_hi as u32,
            superblock,
        )
    }

    pub fn get_inode_bitmap_csum(&self, superblock: &Superblock) -> u64 {
        Self::get_64bit_extended_val(
            self.bg_inode_bitmap_csum_lo as u32,
            self.bg_inode_bitmap_csum_hi as u32,
            superblock,
        )
    }

    pub fn get_itable_unused(&self, superblock: &Superblock) -> u64 {
        Self::get_64bit_extended_val(
            self.bg_itable_unused_lo as u32,
            self.bg_itable_unused_hi as u32,
            superblock,
        )
    }

    pub fn set_free_inodes_count(&mut self, val: u64, superblock: &Superblock) {
        // TODO(Bobby): have this add a deferred write
        Self::set_64bit_extended_val_u16(
            val,
            &mut self.bg_free_inodes_count_lo,
            &mut self.bg_free_inodes_count_hi,
            superblock,
        );
    }

    pub fn set_free_blocks_count(&mut self, val: u64, superblock: &Superblock) {
        // TODO(Bobby): have this add a deferred write
        Self::set_64bit_extended_val_u16(
            val,
            &mut self.bg_free_blocks_count_lo,
            &mut self.bg_free_blocks_count_hi,
            superblock,
        );
    }
}

pub mod bg_flags {
    const EXT4_BG_INODE_UNINIT: u16 = 0x1;
    const EXT4_BG_BLOCK_UNINIT: u16 = 0x2;
    const EXT4_BG_INODE_ZEROED: u16 = 0x4;
}
