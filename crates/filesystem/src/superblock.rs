#[repr(C, packed(4))]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Superblock {
    pub s_inodes_count: u32,
    pub s_blocks_count: u32,
    pub s_r_blocks_count: u32,
    pub s_free_blocks_count: u32,
    pub s_free_inodes_count: u32,
    pub s_first_data_block: u32,
    pub s_log_block_size: u32,
    pub s_log_cluster_size: u32,
    pub s_blocks_per_group: u32,
    pub s_clusters_per_group: u32,
    pub s_inodes_per_group: u32,
    pub s_mtime: u32,
    pub s_wtime: u32,
    pub s_mnt_count: u16,
    pub s_max_mnt_count: u16,
    pub s_magic: u16,
    pub s_state: u16,
    pub s_errors: u16,
    pub s_minor_rev_level: u16,
    pub s_lastcheck: u32,
    pub s_checkinterval: u32,
    pub s_creator_os: u32,
    pub s_rev_level: u32,
    pub s_def_resuid: u16,
    pub s_def_resgid: u16,
    pub s_first_ino: u32,
    pub s_inode_size: u16,
    pub s_block_group_nr: u16,
    pub s_feature_compat: u32,
    pub s_feature_incompat: u32,
    pub s_feature_ro_compat: u32,
    pub s_uuid: [u32; 4],
    pub s_volume_name: [u32; 4],
    pub s_last_mounted: [u32; 16],
    pub s_algorithm_usage_bitmap: u32,
    pub s_prealloc_blocks: u8,
    pub s_prealloc_dir_blocks: u8,
    pub s_reserved_gdt_blocks: u16,
    pub s_journal_uuid: [u32; 4],
    pub s_journal_inum: u32,
    pub s_journal_dev: u32,
    pub s_last_orphan: u32,
    pub s_hash_seed: [u32; 4],
    pub s_def_hash_version: u8,
    pub s_jnl_backup_type: u8,
    pub s_desc_size: u16,
    pub s_default_mount_options: u32,
    pub s_first_meta_bg: u32,

    // new fancy ext4 flags wow
    pub s_mkfs_time: u32,
    pub s_jnl_blocks: [u32; 17],
    pub s_blocks_count_hi: u32,
    pub s_r_blocks_count_hi: u32,
    pub s_free_blocks_count_hi: u32,
    pub s_min_extra_isize: u16,
    pub s_want_extra_isize: u16,
    pub s_flags: u32,
    pub s_raid_stride: u16,
    pub s_mmp_interval: u16,
    pub s_mmp_block: u64,
    pub s_raid_stripe_width: u32,
    pub s_log_groups_per_flex: u8,
    pub s_checksum_type: u8,
    pub s_reserved_pad: u16,
    pub s_kbytes_written: u64,
    pub s_snapshot_inum: u32,
    pub s_snapshot_id: u32,
    pub s_snapshot_r_blocks_count: u64,
    pub s_snapshot_list: u32,
    pub s_error_count: u32,
    pub s_first_error_time: u32,
    pub s_first_error_ino: u32,
    pub s_first_error_block: u32,
    pub s_first_error_func: [u32; 8],
    pub s_first_error_line: u32,
    pub s_last_error_time: u32,
    pub s_last_error_ino: u32,
    pub s_last_error_line: u32,
    pub s_last_error_block: u64,
    pub s_last_error_func: [u32; 8],
    pub s_mount_opts: [u32; 16],
    pub s_usr_quota_inum: u32,
    pub s_grp_quota_inum: u32,
    pub s_overhead_blocks: u32,
    pub s_backup_bgs: [u32; 2],
    pub s_encrypt_algos: u32,
    pub s_encrypt_pw_salt: [u32; 4],
    pub s_lpf_ino: u32,
    pub s_prj_quota_inum: u32,
    pub s_checksum_seed: u32,
    pub s_wtime_hi: u8,
    pub s_mtime_hi: u8,
    pub s_mkfs_time_hi: u8,
    pub s_lastcheck_hi: u8,
    pub s_first_error_time_hi: u8,
    pub s_last_error_time_hi: u8,
    pub s_pad: u16,
    pub s_reserved: [u32; 96],
    pub s_checksum: u32,
    pub s_pad_2: u32,
}

impl Superblock {
    pub fn get_num_of_block_groups(&self) -> u32 {
        self.s_inodes_count / self.s_inodes_per_group
    }

    pub fn get_block_size(&self) -> usize {
        1024 << self.s_log_block_size
    }

    pub fn get_flexible_block_groups(&self) -> usize {
        assert_eq!(
            self.s_feature_incompat & s_feature_incompat::EXT4_FEATURE_INCOMPAT_FLEX_BG,
            0
        );

        let base: usize = 2;

        base.pow(self.s_log_groups_per_flex as u32)
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
    const EXT2_FEATURE_COMPAT_DIR_PREALLOC: u32 = 0x0001;
    const EXT2_FEATURE_COMPAT_IMAGIC_INODES: u32 = 0x0002;
    const EXT3_FEATURE_COMPAT_HAS_JOURNAL: u32 = 0x0004;
    const EXT2_FEATURE_COMPAT_EXT_ATTR: u32 = 0x0008;
    const EXT2_FEATURE_COMPAT_RESIZE_INODE: u32 = 0x0010;
    const EXT2_FEATURE_COMPAT_DIR_INDEX: u32 = 0x0020;
    const EXT4_FEATURE_COMPAT_COMPAT_LAZY_BG: u32 = 0x0040; // this could be ext3
    const EXT4_FEATURE_COMPAT_EXCLUDE_INODE: u32 = 0x0080; // this could be ext3
    const EXT4_FEATURE_COMPAT_EXCLUDE_BITMAP: u32 = 0x0100;
    const EXT4_FEATURE_COMPAT_SPARSE_SUPER2: u32 = 0x0200;
}

pub mod s_feature_incompat {
    pub(crate) const EXT2_FEATURE_INCOMPAT_COMPRESSION: u32 = 0x0001;
    pub(crate) const EXT2_FEATURE_INCOMPAT_FILETYPE: u32 = 0x0002;
    pub(crate) const EXT3_FEATURE_INCOMPAT_RECOVER: u32 = 0x0004;
    pub(crate) const EXT3_FEATURE_INCOMPAT_JOURNAL_DEV: u32 = 0x0008;
    pub(crate) const EXT2_FEATURE_INCOMPAT_META_BG: u32 = 0x0010;
    pub(crate) const EXT4_FEATURE_INCOMPAT_EXTENTS: u32 = 0x0040;
    pub(crate) const EXT4_FEATURE_INCOMPAT_64BIT: u32 = 0x0080;
    pub(crate) const EXT4_FEATURE_INCOMPAT_MMP: u32 = 0x0100;
    pub(crate) const EXT4_FEATURE_INCOMPAT_FLEX_BG: u32 = 0x0200;
    pub(crate) const EXT4_FEATURE_INCOMPAT_EA_INODE: u32 = 0x0400;
    pub(crate) const EXT4_FEATURE_INCOMPAT_DIRDATA: u32 = 0x1000;
    pub(crate) const EXT4_FEATURE_INCOMPAT_CSUM_SEED: u32 = 0x2000;
    pub(crate) const EXT4_FEATURE_INCOMPAT_LARGEDIR: u32 = 0x4000;
    pub(crate) const EXT4_FEATURE_INCOMPAT_INLINE_DATA: u32 = 0x8000;
    pub(crate) const EXT4_FEATURE_INCOMPAT_ENCRYPT: u32 = 0x10000;
}

pub mod s_feature_ro_compat {
    pub(crate) const EXT2_FEATURE_RO_COMPAT_SPARSE_SUPER: u32 = 0x0001;
    pub(crate) const EXT2_FEATURE_RO_COMPAT_LARGE_FILE: u32 = 0x0002;
    pub(crate) const EXT2_FEATURE_RO_COMPAT_BTREE_DIR: u32 = 0x0004;
    pub(crate) const EXT4_RO_COMPAT_HUGE_FILE: u32 = 0x0008;
    pub(crate) const EXT4_RO_COMPAT_GDT_CSUM: u32 = 0x0010;
    pub(crate) const EXT4_RO_COMPAT_DIR_NLINK: u32 = 0x0020;
    pub(crate) const EXT4_RO_COMPAT_EXTRA_ISIZE: u32 = 0x0040;
    pub(crate) const EXT4_RO_COMPAT_HAS_SNAPSHOT: u32 = 0x0080;
    pub(crate) const EXT4_RO_COMPAT_QUOTA: u32 = 0x0100;
    pub(crate) const EXT4_RO_COMPAT_METADATA_CSUM: u32 = 0x0200;
    pub(crate) const EXT4_RO_COMPAT_REPLICA: u32 = 0x0800;
    pub(crate) const EXT4_RO_COMPAT_READONLY: u32 = 0x1000;
    pub(crate) const EXT4_RO_COMPAT_PROJECT: u32 = 0x2000;
}

pub mod s_algo_bitmap {
    pub const EXT2_LZV1_ALG: u32 = 0x0001;
    pub const EXT2_LZRW3A_ALG: u32 = 0x0002;
    pub const EXT2_GZIP_ALG: u32 = 0x0004;
    pub const EXT2_BZIP2_ALG: u32 = 0x0008;
    pub const EXT2_LZO_ALG: u32 = 0x0010;
}

pub mod s_def_hash_version {
    const EXT4_LEGACY_HASH: u8 = 0x0;
    const EXT4_HALF_MD4_HASH: u8 = 0x1;
    const EXT4_TEA_HASH: u8 = 0x2;
    const EXT4_LEGACY_UNSIGNED_HASH: u8 = 0x3;
    const EXT4_HALF_MD4_UNSIGNED_HASH: u8 = 0x4;
    const EXT4_TEA_UNSIGNED_HASH: u8 = 0x5;
}

pub mod s_default_mount_options {
    const EXT4_DEFM_DEBUG: u32 = 0x0001;
    const EXT4_DEFM_BSDGROUPS: u32 = 0x0002;
    const EXT4_DEFM_XATTR_USER: u32 = 0x0004;
    const EXT4_DEFM_ACL: u32 = 0x0008;
    const EXT4_DEFM_UID16: u32 = 0x0010;
    const EXT4_DEFM_JMODE_DATA: u32 = 0x0020;
    const EXT4_DEFM_JMODE_ORDERED: u32 = 0x0040;
    const EXT4_DEFM_JMODE_WBACK: u32 = 0x0060;
    const EXT4_DEFM_NOBARRIER: u32 = 0x0100;
    const EXT4_DEFM_BLOCK_VALIDITY: u32 = 0x0200;
    const EXT4_DEFM_DISCARD: u32 = 0x0400;
    const EXT4_DEFM_NODELALLOC: u32 = 0x0800;
}

pub mod s_flags {
    const EXT4_FLAGS_SIGNED_DIR_HASH: u32 = 0x0001;
    const EXT4_FLAGS_UNSIGNED_DIR_HASH: u32 = 0x0002;
    const EXT4_FLAGS_TESTING_DEV_CODE: u32 = 0x0004;
}

pub mod s_encrypt_algos {
    const EXT4_ENCRYPTION_MODE_INVALID: u32 = 0x0000;
    const EXT4_ENCRYPTION_MODE_AES_256_XTS: u32 = 0x0001;
    const EXT4_ENCRYPTION_MODE_AES_256_GCM: u32 = 0x0002;
    const EXT4_ENCRYPTION_MODE_AES_256_CBC: u32 = 0x0003;
}

const _: () = assert!(size_of::<Superblock>() == 1024);
