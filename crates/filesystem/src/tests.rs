use alloc::rc::Rc;
use std::cell::RefCell;
use crate::{linux::FileBlockDevice, INodeWrapper, Superblock};
use std::fs::File;
use std::io;
use std::prelude::v1::{Box, String, ToString};
use std::process::{Command, Output};
use crate::{BlockDevice, Ext2};
fn create_test_image_artifact(dir_path: String, block_size: usize, img_name: String) {
    Command::new("mkfs.ext2")
        .args(["-q", "-b", &*block_size.to_string(), "-i",
                &*block_size.to_string(), "-d", &*dir_path, "-I",
               "128", "-r", "0", "-t", "ext2", &*img_name, "10m"]).output().unwrap();
}

#[test]
fn read_example_1() {
    create_test_image_artifact("../../test/example_1.dir".parse().unwrap(), 1024,
                               "example_1_ro.img".parse().unwrap());

    let file = File::open("example_1_ro.img").unwrap();
    let disk = FileBlockDevice::new(file);

    let mut ext2 = Ext2::new(disk);

    assert_eq!(ext2.get_block_size(), 1024);
    assert_eq!(ext2.get_inode_size(), 128);
    
    let root_node = ext2.get_root_inode_wrapper();

    let test_node_binding = ext2.find(&root_node.borrow(), b"test.txt").unwrap();
    let test_node = test_node_binding.borrow();
    let test_node_text: String = test_node.read_text_file_as_str(&mut ext2).unwrap();

    assert_eq!(test_node_text, "asldfalsjdkfvnlasdfvnka,dsfvmna");

    let test_folder_binding = ext2.find(&root_node.borrow(), b"folder").unwrap();
    
    let test_folder = test_folder_binding.borrow();

    let test_file_in_folder_binding = ext2.find(&test_folder, b"asdf.txt").unwrap();
    let test_file_in_folder = test_file_in_folder_binding.borrow();
    let test_file_in_folder_text: String = 
        test_file_in_folder.read_text_file_as_str(&mut ext2).unwrap();

    assert_eq!(test_file_in_folder_text, "Hi");
}

#[test]
fn read_write_example_1() {
    create_test_image_artifact("../../test/example_1.dir".parse().unwrap(), 1024,
                               "example_1_rw.img".parse().unwrap());

    let file = File::options().read(true).write(true).open("example_1_rw.img").unwrap();
    let disk = FileBlockDevice::new(file);

    let mut ext2 = Ext2::new(disk);

    assert_eq!(ext2.get_block_size(), 1024);
    assert_eq!(ext2.get_inode_size(), 128);

    let root_node: Rc<RefCell<INodeWrapper>> = ext2.get_root_inode_wrapper();

    let test_folder_binding = ext2.find(&root_node.borrow(), b"folder").unwrap();
    let test_folder = test_folder_binding.borrow();
    let test_file_in_folder_binding = ext2.find(&test_folder, b"asdf.txt").unwrap();
    let mut test_file_in_folder = test_file_in_folder_binding.borrow_mut();
    let test_file_in_folder_text: String =
        test_file_in_folder.read_text_file_as_str(&mut ext2).unwrap();

    assert_eq!(test_file_in_folder_text, "Hi");

    let new_append_string = b"Hi";

    test_file_in_folder.append_file(&mut ext2, new_append_string, true).unwrap();

    let test_file_in_folder_text: String =
        test_file_in_folder.read_text_file_as_str(&mut ext2).unwrap();

    assert_eq!(test_file_in_folder_text, "HiHi")
}

#[test]
fn file_creation_test() {
    create_test_image_artifact("../../test/example_1.dir".parse().unwrap(), 1024,
                               "example_1_rw_file_creation.img".parse().unwrap());

    let file = File::options().read(true).write(true).open("example_1_rw_file_creation.img").unwrap();
    let disk = FileBlockDevice::new(file);

    let mut ext2 = Ext2::new(disk);
    let root_node = ext2.get_root_inode_wrapper();
    
    let new_file = ext2.create_file(&mut root_node.borrow_mut(),
                                                     b"weeee.txt").unwrap();
    let new_file_copy = ext2.find(&root_node.borrow(),
                                                   b"weeee.txt").unwrap();

    let test_string = b"Ding Ding Arriving outbound 2-car L to San Francisco Zoo";
    
    assert_eq!(new_file_copy.borrow().inode_num, new_file.borrow().inode_num);

    new_file.borrow_mut().append_file(&mut ext2, test_string,true).unwrap();

    let read_back_data = new_file_copy.borrow().read_file(&mut ext2).unwrap(); 
    
    assert_eq!(read_back_data.as_slice(), test_string);
}

#[test]
fn file_creation_and_rw_test() {
    //
}

#[test]
fn directory_creation_test() {
    //
}
