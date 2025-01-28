#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use device::uart::UART;
use kernel::*;

#[no_mangle]
extern "Rust" fn kernel_main(device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    println!("Reserved regions:");
    for region in device_tree.reserved_regions {
        println!(
            "  {:#010} (size {:x})",
            region.address.get(),
            region.size.get()
        );
    }

    println!("Device tree:");

    device_tree::debug_device_tree(&device_tree, &mut UART.get()).unwrap();
}
