#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::device::system_timer::micro_delay;
use kernel::*;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    // Basic keyboard test
    loop {
        micro_delay(10000);
        while let Some(event) = device::usb::keyboard::KEY_EVENTS.poll() {
            match event.pressed {
                true => println!("Key {:?} ({}) pressed", event.key, event.code),
                false => println!("Key {:?} ({}) released", event.key, event.code),
            }
        }
    }
}
