#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use device::{discover_compatible, find_device_addr, mailbox};
use kernel::*;

#[no_mangle]
extern "Rust" fn kernel_main(tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    let mailbox = discover_compatible(&tree, b"brcm,bcm2835-mbox")
        .unwrap()
        .next()
        .unwrap();
    let (mailbox_addr, _) = find_device_addr(mailbox).unwrap().unwrap();
    let mailbox_base = unsafe { memory::map_device(mailbox_addr) }.as_ptr();
    let mut mailbox = unsafe { mailbox::VideoCoreMailbox::init(mailbox_base) };
}
