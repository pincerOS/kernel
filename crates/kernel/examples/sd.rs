#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use device::{discover_compatible, find_device_addr, sdcard};
use kernel::{
    device::sdcard::SD,
    sync::SpinLock,
    *,
};

#[no_mangle]
extern "Rust" fn kernel_main(tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

     // Should use this on hardware
    // let sdcard = discover_compatible(&tree, b"brcm,bcm2711-emmc2")
    //         .unwrap()
    //         .next()
    //         .unwrap();
    // The bcm2835-sdhci requires additional gpio pin initialization which could possibly conflict with other drivers that need those pinss
    let sdcard = discover_compatible(&tree, b"brcm,bcm2835-sdhci")
        .unwrap()
        .next()
        .unwrap();
    let (sdcard_addr, _) = find_device_addr(sdcard).unwrap().unwrap();
    let sdcard_base = unsafe { memory::map_device(sdcard_addr) }.as_ptr();
    println!("| SD Card controller addr: {:#010x}", sdcard_addr as usize);
    println!("| SD Card controller base: {:#010x}", sdcard_base as usize);
    let sdcard = unsafe { sdcard::bcm2711_emmc2_driver::init(sdcard_base) };
    unsafe { SD.init(SpinLock::new(sdcard)) };
    println!("| initialized SD Card");
}
