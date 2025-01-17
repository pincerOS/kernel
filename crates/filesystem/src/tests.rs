use crate::{linux::FileBlockDevice, Superblock};
use std::fs::File;

use crate::{BlockDevice, Ext2};

#[test]
fn example_1() {
    let file = File::open("example_1.img").unwrap();
    let disk = FileBlockDevice::new(file);

    let mut ext2 = Ext2::new(disk);

    assert_eq!(ext2.get_block_size(), 1024);
    assert_eq!(ext2.get_inode_size(), 128);
}
