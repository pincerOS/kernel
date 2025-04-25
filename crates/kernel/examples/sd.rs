#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use device::{discover_compatible, find_device_addr, sdcard};
use filesystem::BlockDevice;
use kernel::{device::sdcard::SD, sync::SpinLock, *};

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

    let mut sd = SD.get().lock();
    let mut buf = [0u8; 512];
    let mut buf1024 = [7u8; 1024];
    let buf0 = [0u8; 512];
    let buf1 = [1u8; 512];

    println!("Reading block 1");
    sd.read_sector(1, &mut buf).unwrap();
    println!("{:?}", buf);
    println!("Writing 1 to block 1");
    sd.write_sector(1, &buf1).unwrap();
    println!("Reading block 1");
    sd.read_sector(1, &mut buf).unwrap();
    println!("{:?}", buf);
    assert_eq!(buf, buf1);
    println!("Reading block 0");
    sd.read_sector(0, &mut buf).unwrap();
    println!("{:?}", buf);
    println!("Writing 1 to block 0");
    sd.write_sector(0, &buf1).unwrap();
    println!("Reading block 0");
    sd.read_sector(0, &mut buf).unwrap();
    println!("{:?}", buf);
    assert_eq!(buf, buf1);
    println!("Reading block 0 & 1");
    sd.read_sectors(0, &mut buf1024).unwrap();
    println!("{:?}", buf1024);
    assert_eq!(buf1024[0..512], buf1);
    assert_eq!(buf1024[512..1024], buf1);
    println!("Reading block 2");
    sd.read_sector(2, &mut buf).unwrap();
    println!("{:?}", buf);

    println!("Writing 0 to block 0 & 1");
    sd.write_sector(0, &buf0).unwrap();
    sd.write_sector(1, &buf0).unwrap();

    println!("Reading block 0 & 1");
    sd.read_sectors(0, &mut buf1024).unwrap();
    println!("{:?}", buf1024);
    assert_eq!(buf1024[0..512], buf0);
    assert_eq!(buf1024[512..1024], buf0);
}
