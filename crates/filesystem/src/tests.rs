#[cfg(feature = "std")]
extern crate std;

use crate::{linux::FileBlockDevice, Ext2, INodeWrapper};
use alloc::rc::Rc;
use std::fs::File;

#[test]
fn example_1() {
    let file = File::open("example_1.img").unwrap();
    let disk = FileBlockDevice::new(file);

    let mut ext2 = Ext2::new(disk).unwrap();

    assert_eq!(ext2.get_block_size(), 1024);
    assert_eq!(ext2.get_inode_size(), 128);

    let root_node: Rc<INodeWrapper> = ext2.get_root_inode_wrapper();

    let test_node: Rc<INodeWrapper> = ext2.find(&root_node, "test.txt").unwrap();
    let test_node_text = test_node.read_text_file_as_str(&mut ext2).unwrap();

    assert_eq!(test_node_text, "asldfalsjdkfvnlasdfvnka,dsfvmna");

    let test_folder: Rc<INodeWrapper> = ext2.find(&root_node, "folder").unwrap();
    let test_file_in_folder = ext2.find(&test_folder, "asdf.txt").unwrap();
    let test_file_in_folder_text = test_file_in_folder
        .read_text_file_as_str(&mut ext2)
        .unwrap();

    assert_eq!(test_file_in_folder_text, "Hi");
}
