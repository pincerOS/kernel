#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use device::usb::mouse::{MouseEvent, MOUSE_EVENTS};
use kernel::device::usb::keyboard::Key;
use kernel::*;
use sync::time::sleep;

use kernel::event::thread;
use kernel::*;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");
    crate::event::task::spawn_async(async move {
        main().await;
    });
    crate::event::thread::stop();
}

async fn main() {
    // Basic loop, lets us wait for networking packets
    loop {
        sleep(10000).await;
    }
}
