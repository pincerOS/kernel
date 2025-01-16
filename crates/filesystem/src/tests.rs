use crate::{linux::FileBlockDevice, Superblock};
use std::fs::File;

use crate::{BlockDevice, Ext2};

#[test]
fn example_1() {
    let file = File::open("example_1.img").unwrap();
    let disk = FileBlockDevice::new(file);

    let ext2 = Ext2::new(disk);

    
}
