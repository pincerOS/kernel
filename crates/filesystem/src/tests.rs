use alloc::rc::Rc;
use crate::{linux::FileBlockDevice, INodeWrapper, Superblock};
use std::fs::File;

use crate::{BlockDevice, Ext2};

#[test]
fn read_example_1() {
    let file = File::open("example_1_ro.img").unwrap();
    let disk = FileBlockDevice::new(file);

    let mut ext2 = Ext2::new(disk);

    assert_eq!(ext2.get_block_size(), 1024);
    assert_eq!(ext2.get_inode_size(), 128);
    
    let root_node = ext2.get_root_inode_wrapper();

    let test_node_binding = ext2.find(&root_node, "test.txt").unwrap();
    let test_node = test_node_binding.borrow();
    let test_node_text: std::string::String = test_node.read_text_file_as_str(&mut ext2);

    assert_eq!(test_node_text, "asldfalsjdkfvnlasdfvnka,dsfvmna");

    let test_folder_binding = ext2.find(&root_node, "folder").unwrap();
    
    let test_folder = test_folder_binding.borrow();

    let test_file_in_folder_binding = ext2.find(&test_folder, "asdf.txt").unwrap();
    let test_file_in_folder = test_file_in_folder_binding.borrow();
    let test_file_in_folder_text: std::string::String = 
        test_file_in_folder.read_text_file_as_str(&mut ext2);

    assert_eq!(test_file_in_folder_text, "Hi");
}

#[test]
fn read_write_example_1() {
    let file = File::options().read(true).write(true).open("example_1_rw.img").unwrap();
    let disk = FileBlockDevice::new(file);

    let mut ext2 = Ext2::new(disk);

    assert_eq!(ext2.get_block_size(), 1024);
    assert_eq!(ext2.get_inode_size(), 128);

    let root_node: Rc<INodeWrapper> = ext2.get_root_inode_wrapper();

    let test_folder_binding = ext2.find(&root_node, "folder").unwrap();
    let test_folder = test_folder_binding.borrow();
    let test_file_in_folder_binding = ext2.find(&test_folder, "asdf.txt").unwrap();
    let mut test_file_in_folder = test_file_in_folder_binding.borrow_mut();
    let test_file_in_folder_text: std::string::String =
        test_file_in_folder.read_text_file_as_str(&mut ext2);

    assert_eq!(test_file_in_folder_text, "Hi");

    let new_append_string = b"Hi";

    test_file_in_folder.append_file(&mut ext2, new_append_string, true).unwrap();

    let test_file_in_folder_text: std::string::String =
        test_file_in_folder.read_text_file_as_str(&mut ext2);

    assert_eq!(test_file_in_folder_text, "HiHi")
}
