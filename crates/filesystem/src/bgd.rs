#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BGD {
    pub(crate) bg_block_bitmap: u32,
    pub(crate) bg_inode_bitmap: u32,
    pub(crate) bg_inode_table: u32,
    pub(crate) bg_free_blocks_count: u16,
    pub(crate) bg_free_inodes_count: u16,
    pub(crate) bg_used_dirs_count: u16,
    pub(crate) bg_pad: u16,
    pub(crate) bg_reserved: [u8; 12], /*bg_flags: u16,
                                      bg_exclude_bitmap_lo: u32,
                                      bg_block_bitmap_csum_lo: u16,
                                      bg_inode_bitmap_csum_lo: u16,
                                      bg_itable_unused_lo: u16,
                                      bg_checksum: u16,
                                      bg_block_bitmap_hi: u32,
                                      bg_inode_bitmap_hi: u32,
                                      bg_free_blocks_count_hi: u16,
                                      bg_free_inodes_count_hi: u16,
                                      bg_used_dirs_count_hi: u16,
                                      bg_itable_unused_hi: u16,
                                      bg_exclude_bitmap_hi: u32,
                                      bg_block_bitmap_csum_hi: u16,
                                      bg_inode_bitmap_csum_hi: u16,
                                      bg_pad: u32,*/
}

pub mod bg_flags {
    const EXT4_BG_INODE_UNINIT: u16 = 0x1;
    const EXT4_BG_BLOCK_UNINIT: u16 = 0x2;
    const EXT4_BG_INODE_ZEROED: u16 = 0x4;
}
