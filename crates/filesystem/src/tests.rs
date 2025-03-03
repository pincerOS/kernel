#[cfg(feature = "std")]
extern crate std;

use crate::{linux::FileBlockDevice, Ext2, INodeWrapper};
use alloc::rc::Rc;
use std::cell::RefCell;
use std::{format, fs};
use std::fs::{File, ReadDir};
use std::{env, io, println, vec};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::prelude::v1::{Box, String, ToString, Vec};
use std::process::{Command, Output};

pub fn create_ext2_fs(dir_path: &str, block_size: usize, img_name: &str, ro: bool) -> Ext2<FileBlockDevice> {
    Command::new("mkfs.ext2")
        .args(["-q", "-b", &*block_size.to_string(), "-i",
            &*block_size.to_string(), "-d", &*dir_path, "-I",
            "128", "-r", "0", "-t", "ext2", &*img_name, "64m"]).output().unwrap();

    let file: File = File::options().read(true).write(!ro).open(img_name).unwrap();
    let disk: FileBlockDevice = FileBlockDevice::new(file);

    let mut ext2 = Ext2::new(disk);

    ext2.unwrap()
}

#[derive(PartialEq, Eq)]
enum WriteMode {
    None,
    Write,
    Append,
    CreateWrite
}

struct VerifyRequest<'a> {
    file_path: &'a [u8],
    data: &'a [u8],
    expect_data: Option<&'a [u8]>,
    write_mode: WriteMode,
    create_dirs_if_nonexistent: bool
}

fn unmount_fuse_fs(mount_dir: &str) {
    let unmount_result = Command::new("umount").args([mount_dir]).output().unwrap();
    let unmount_stderr = String::from_utf8_lossy(&unmount_result.stderr);
    
    println!("{}", unmount_stderr);
}

fn read_and_verify_via_fuse_test(verify_requests: &Vec<VerifyRequest>,
                                 image_path: &str) {
    let mut test_mount_dir = String::from("/tmp/tst-fuse-ext2-mnt-");
    test_mount_dir.push_str(image_path);

    let mkdir_result = Command::new("mkdir").args([test_mount_dir.as_str()]).output();
    let mut complete_image_path = String::from(std::env::current_dir().unwrap().to_str().unwrap());

    complete_image_path.push_str("/");
    complete_image_path.push_str(image_path);

    let fuse_ext2_output = Command::new("fuse-ext2")
        .args([complete_image_path.as_str(), &*test_mount_dir, "-o", "rw+,allow_other,nonempty,uid=501,gid=20"]).output().unwrap();
    let fuse_ext2_stderr = String::from_utf8(fuse_ext2_output.stderr).unwrap();
    
    assert!(fuse_ext2_output.status.success());

    let mut dir_trees: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for verify_request in verify_requests {
        let mut current_file_path: String = test_mount_dir.clone();
        let mut file_bytes: Vec<u8> = Vec::new();
        let expected_data: Vec<u8> = if verify_request.expect_data.is_some() {
            verify_request.expect_data.unwrap().to_vec()
        } else {
            verify_request.data.to_vec()
        };

        let file_path_string_slice: &str = std::str::from_utf8(verify_request.file_path).unwrap();

        current_file_path.push('/');
        current_file_path.push_str(file_path_string_slice);

        let verify_path = Path::new(&current_file_path).parent().unwrap();

        if !dir_trees.contains_key(verify_path.to_str().unwrap()) {
            let mut dir_files: BTreeSet<String> = BTreeSet::new();

            for path in fs::read_dir(verify_path).unwrap() {
                let path_string = path.unwrap().path().to_str().unwrap().to_string();

                dir_files.insert(path_string);
            }

            dir_trees.insert(verify_path.to_str().unwrap().to_string(), dir_files);
        }

        // need to check that file is accessible from directory scans, not just that
        // we can read from them.
        let dir_tree: &BTreeSet<String> =
            dir_trees.get(&verify_path.to_str().unwrap().to_string()).unwrap();

        assert!(dir_tree.contains(current_file_path.as_str()));
        
        println!("current file path {}", current_file_path);
        
        File::open(current_file_path).unwrap().read_to_end(&mut file_bytes).unwrap();

        if expected_data != file_bytes.as_slice() {
            unmount_fuse_fs(test_mount_dir.as_str());
            assert_eq!(expected_data, file_bytes);
        }
    }

    unmount_fuse_fs(test_mount_dir.as_str());
}

fn read_and_verify_test(ext2: &mut Ext2<FileBlockDevice>, verify_requests: &Vec<VerifyRequest>,
                        image_path: &str) {
    let root_node: Rc<RefCell<INodeWrapper>> = ext2.get_root_inode_wrapper();
    
    for (i, verify_request) in verify_requests.iter().enumerate() {
        let file_node: Rc<RefCell<INodeWrapper>> =
            ext2.find_recursive(root_node.clone(), verify_request.file_path, false, false).unwrap();

        let file_bytes: Vec<u8> = file_node.borrow().read_file(ext2).unwrap();
        let expected_data: Vec<u8> = if verify_request.expect_data.is_some() {
            verify_request.expect_data.unwrap().to_vec()
        } else {
            verify_request.data.to_vec()
        };

        assert_eq!(file_bytes, expected_data);
    }

    if env::consts::OS == "linux" {
        //read_and_verify_via_fuse_test(verify_requests, image_path);
    }
}

fn write_and_verify_test(ext2: &mut Ext2<FileBlockDevice>, verify_requests: &Vec<VerifyRequest>,
                         image_path: &str) {
    let root_node = ext2.get_root_inode_wrapper();

    for verify_request in verify_requests {
        let file_path_str: &str = std::str::from_utf8(verify_request.file_path).unwrap();

        let file_node: Rc<RefCell<INodeWrapper>> =
            ext2.find_recursive(root_node.clone(), verify_request.file_path,
                                verify_request.create_dirs_if_nonexistent,
                                verify_request.write_mode == WriteMode::CreateWrite).unwrap();

        match verify_request.write_mode {
            WriteMode::CreateWrite | WriteMode::Write => {
                file_node.borrow_mut().overwrite_file(ext2, verify_request.data, true).unwrap();
            }
            WriteMode::Append => {
                file_node.borrow_mut().append_file(ext2, verify_request.data, true).unwrap();
            }
            WriteMode::None => {}
        }
    }

    read_and_verify_test(ext2, verify_requests, image_path);
}

#[test]
fn read_example_1() {
    let image_path = "ro.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, true);

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"test.txt",
            data: b"asldfalsjdkfvnlasdfvnka,dsfvmna",
            expect_data: None,
            write_mode: WriteMode::None,
            create_dirs_if_nonexistent: false
        },
        VerifyRequest {
            file_path: b"folder/asdf.txt",
            data: b"Hi",
            expect_data: None,
            write_mode: WriteMode::None,
            create_dirs_if_nonexistent: false
        }
    ];

    read_and_verify_test(&mut ext2, &verify_requests, image_path);
}

#[test]
fn read_write_example_1() {
    let image_path = "rw.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, false);

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"folder/asdf.txt",
            data: b"Hi",
            expect_data: Some(b"HiHi"),
            write_mode: WriteMode::Append,
            create_dirs_if_nonexistent: false
        }
    ];

    write_and_verify_test(&mut ext2, &verify_requests, image_path);
}

const GENERATED_TEST_DIR_ROOT: &str = "../../test/generated_tests";

// from Simon Buchan on stack overflow: https://stackoverflow.com/a/65192210
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn create_new_test_folder(test_folder_name: &str, base_folder: Option<&str>) -> PathBuf {
    let path: PathBuf = [GENERATED_TEST_DIR_ROOT, test_folder_name].iter().collect();

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }

    copy_dir_all(base_folder.unwrap(), &path).unwrap();

    path
}

#[test]
fn single_indirect_read_test() {
    let test_folder_path: PathBuf = create_new_test_folder("single_indirect_read_test",
                                                           Some("../../test/example_1.dir"));
    let mut test_file_path: PathBuf = test_folder_path.clone();
    test_file_path.push("single.txt");

    let mut f = File::create_new(test_file_path).unwrap();
    let mut text_bytes = vec![0; 1024*13];
    for i in 0..13 {
        text_bytes[i*1024] = (i+1) as u8;
    }
    f.write(&text_bytes);

    // block size is 1024 for now
    let image_path = "indirect1.img";
    let mut ext2 =
        create_ext2_fs(test_folder_path.to_str().unwrap(), 1024, image_path, true);

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"single.txt",
            data: &text_bytes,
            expect_data: None,
            write_mode: WriteMode::None,
            create_dirs_if_nonexistent: false
        }
    ];
    fs::remove_file("../../test/example_1.dir/single.txt");

    read_and_verify_test(&mut ext2, &verify_requests, image_path);
}

#[test]
fn double_indirect_read_test() {
    let test_folder_path: PathBuf = create_new_test_folder("double_indirect_read_test",
                                                           Some("../../test/example_1.dir"));
    let mut test_file_path: PathBuf = test_folder_path.clone();
    test_file_path.push("double.txt");

    let mut f = File::create_new(test_file_path).unwrap();
    let mut text_bytes = vec![0; 1024*269];
    for i in 0..269 {
        text_bytes[i*1024] = (i+1) as u8;
    }
    f.write(&text_bytes);

    // block size is 1024 for now
    let image_path = "indirect2.img";
    let mut ext2 =
        create_ext2_fs(test_folder_path.to_str().unwrap(), 1024, image_path, true);

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"double.txt",
            data: &text_bytes,
            expect_data: None,
            write_mode: WriteMode::None,
            create_dirs_if_nonexistent: false
        }
    ];
    fs::remove_file("../../test/example_1.dir/double.txt");

    read_and_verify_test(&mut ext2, &verify_requests, image_path);
}

// TODO(Sasha): This test doesn't work yet and is also really slow so 
// #[test]
// fn triple_indirect_read_test() {
//     let mut f = File::create_new("../../test/example_1.dir/triple.txt").unwrap();
//     let mut text_bytes = vec![0; 1024*65805];
//     for i in 0..65805 {
//         text_bytes[i*1024] = (i+1) as u8;
//     }
//     f.write(&text_bytes);

//     // block size is 1024 for now
//     let mut ext2 =
//         create_ext2_fs("../../test/example_1.dir", 1024, "indirect3.img", true);

//     let verify_requests = vec![
//         VerifyRequest {
//             file_path: b"triple.txt",
//             data: &text_bytes,
//             expect_data: None,
//             write_mode: WriteMode::None,
//             create_dirs_if_nonexistent: false
//         }
//     ];
//     fs::remove_file("../../test/example_1.dir/triple.txt");

//     read_and_verify_test(&mut ext2, &verify_requests);
// }

#[test]
fn append_alot_test() {
    let image_path = "append-alot.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, false);

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"folder/asdf.txt",
            data: b"HiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHi",
            expect_data: Some(b"HiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHiHi"),
            write_mode: WriteMode::Append,
            create_dirs_if_nonexistent: false
        }
    ];

    write_and_verify_test(&mut ext2, &verify_requests, image_path);
}

#[test]
fn file_overwrite_test() {
    let image_path = "rw_file_overwrite.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, false);

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"folder/asdf.txt",
            data: b"Bye",
            expect_data: Some(b"Bye"),
            write_mode: WriteMode::Write,
            create_dirs_if_nonexistent: false
        }
    ];

    write_and_verify_test(&mut ext2, &verify_requests, image_path);
}

#[test]
fn file_overwrite_moreblocks_test() {
    let image_path = "rw_file_overwrite_moreblocks.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, false);

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"folder/asdf.txt",
            data: b"Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_",
            expect_data: Some(b"Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_Bye_"),
            write_mode: WriteMode::Write,
            create_dirs_if_nonexistent: false
        }
    ];

    write_and_verify_test(&mut ext2, &verify_requests, image_path);
}

#[test]
fn file_overwrite_lessblocks_test() {
    let image_path = "rw_file_overwrite_lessblocks.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, false);

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"folder/asdf.txt",
            data: b"Hi",
            expect_data: Some(b"Hi"),
            write_mode: WriteMode::Write,
            create_dirs_if_nonexistent: false
        }
    ];

    write_and_verify_test(&mut ext2, &verify_requests, image_path);
}

#[test]
fn file_creation_test() {
    let image_path = "rw_file_creation.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, false);

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"weeee.txt",
            data: b"Ding Ding Arriving outbound 2-car L to San Francisco Zoo",
            expect_data: None,
            write_mode: WriteMode::CreateWrite,
            create_dirs_if_nonexistent: false
        }
    ];

    write_and_verify_test(&mut ext2, &verify_requests, image_path);
}

#[test]
fn directory_creation_test() {
    let image_path = "rw_dir_creation.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, false);

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"ilovetrains/train.txt",
            data: b"Ding Ding Arriving outbound 2-car L to San Francisco Zoo",
            expect_data: None,
            write_mode: WriteMode::CreateWrite,
            create_dirs_if_nonexistent: true
        }
    ];

    write_and_verify_test(&mut ext2, &verify_requests, image_path);
}

#[test]
fn big_file_create_test() {
    let image_path = "rw_big_file_creation.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, false);
    let mut large_image_bytes: Vec<u8> = Vec::new();

    File::open("../../test/files_to_add/largeimage.png").unwrap().read_to_end(
                &mut large_image_bytes).unwrap();

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"largeimage.png",
            data: &*large_image_bytes,
            expect_data: None,
            write_mode: WriteMode::CreateWrite,
            create_dirs_if_nonexistent: false
        }
    ];

    write_and_verify_test(&mut ext2, &verify_requests, image_path);
}

#[test]
fn file_creation_within_created_dir_test() {
    let mut image_bytes: Vec<u8> = Vec::new();
    let mut text_file_bytes: Vec<u8> = Vec::new();
    
    let image_path = "rw_file_creation_within_created_dir.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, false);

    File::open("../../test/files_to_add/bee_movie.txt").unwrap().read_to_end(
                &mut text_file_bytes).unwrap();
    File::open("../../test/files_to_add/image.jpg").unwrap().read_to_end(
                &mut image_bytes).unwrap();

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"a/b/image.jpg",
            data: &*image_bytes,
            expect_data: None,
            write_mode: WriteMode::CreateWrite,
            create_dirs_if_nonexistent: true
        },
        VerifyRequest {
            file_path: b"a/textfile.txt",
            data: &*text_file_bytes,
            expect_data: None,
            write_mode: WriteMode::CreateWrite,
            create_dirs_if_nonexistent: false
        }
    ];

    write_and_verify_test(&mut ext2, &verify_requests, image_path);
}

#[test]
fn file_mass_creation_test() {
    let image_path = "rw_mass_file_creation.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, false);
    let mut bart_image_bytes: Vec<u8> = Vec::new();
    let mut bee_movie_bytes: Vec<u8> = Vec::new();

    File::open("../../test/files_to_add/image.jpg").unwrap().read_to_end(
        &mut bart_image_bytes).unwrap();
    File::open("../../test/files_to_add/bee_movie.txt").unwrap().read_to_end(
        &mut bee_movie_bytes).unwrap();

    let mut verify_requests: Vec<VerifyRequest> = Vec::new();
    let mut file_paths: [String; 50] = core::array::from_fn(|i| String::from(""));

    for i in 0..50 {
        let file_suffix: &str = match i % 2 {
            0 => "image.jpg",
            _ => "bee_movie.txt"
        };

        file_paths[i] = format!("{}{}", i, file_suffix);
    }

    for i in 0..50 {
        let data_ptr = match i % 2 {
            0 => &*bart_image_bytes,
            _ => &*bee_movie_bytes,
        };

        verify_requests.push(VerifyRequest {
            file_path: file_paths[i].as_bytes(),
            data: data_ptr,
            expect_data: None,
            write_mode: WriteMode::CreateWrite,
            create_dirs_if_nonexistent: false
        });
    }

    write_and_verify_test(&mut ext2, &verify_requests, image_path);
}

#[test]
fn dir_tree_test() {
    let image_path = "rw_dir_tree.img";
    let mut ext2 =
        create_ext2_fs("../../test/example_1.dir", 1024, image_path, false);
    let mut bart_image_bytes: Vec<u8> = Vec::new();
    let mut bee_movie_bytes: Vec<u8> = Vec::new();
    let mut wmata_image_bytes: Vec<u8> = Vec::new();

    File::open("../../test/files_to_add/image.jpg").unwrap().read_to_end(
        &mut bart_image_bytes).unwrap();
    File::open("../../test/files_to_add/bee_movie.txt").unwrap().read_to_end(
        &mut bee_movie_bytes).unwrap();
    File::open("../../test/files_to_add/largeimage.png").unwrap().read_to_end(
        &mut wmata_image_bytes).unwrap();

    let verify_requests = vec![
        VerifyRequest {
            file_path: b"a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z/image.png",
            data: &*bart_image_bytes,
            expect_data: None,
            write_mode: WriteMode::CreateWrite,
            create_dirs_if_nonexistent: true
        },
        VerifyRequest {
            file_path: b"a/b/c/d/e/f/g/h/i/j/k/l/m/n/bee_movie.txt",
            data: &*bee_movie_bytes,
            expect_data: None,
            write_mode: WriteMode::CreateWrite,
            create_dirs_if_nonexistent: false // directories should already exist at this point
        },
        VerifyRequest {
            file_path: b"a/b/c/image.jpg",
            data: &*wmata_image_bytes,
            expect_data: None,
            write_mode: WriteMode::CreateWrite,
            create_dirs_if_nonexistent: false
        }
    ];

    write_and_verify_test(&mut ext2, &verify_requests, image_path);
}
