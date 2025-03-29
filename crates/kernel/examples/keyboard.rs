#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::*;
use sync::time::sleep;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");
    crate::event::task::spawn_async(async move {
        main().await;
    });
    crate::event::thread::stop();
}

async fn main() {
    // Basic keyboard test
    loop {
        while let Some(event) = device::usb::keyboard::KEY_EVENTS.poll() {
            match event.pressed {
                true => println!("Key {:?} ({}) pressed", event.key, event.code),
                false => println!("Key {:?} ({}) released", event.key, event.code),
            }
        }
        sleep(10000).await;
    }
}
