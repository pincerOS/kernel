#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::device::system_timer::micro_delay;
use kernel::device::usb::device::hid::keyboard::Key;
use kernel::*;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    //Basic keyboard test
    let mut list = device::usb::usb_retrieve_keys();

    loop {
        let new_list = device::usb::usb_retrieve_keys();
        micro_delay(10000);
        for key in new_list.iter() {
            if !list.contains(key) {
                if *key == Key::Return {
                    println!();
                } else {
                    print!("{:?} ", key);
                }
            }
        }
        list = new_list;
    }
}
