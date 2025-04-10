#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use device::{discover_compatible, find_device_addr, mailbox, sdcard};
use kernel::{device::MAILBOX, device::SD, sync::SpinLock, *};

#[no_mangle]
extern "Rust" fn kernel_main(tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    // let sdcard = discover_compatible(&tree, b"brcm,bcm2711-emmc2")
    //         .unwrap()
    //         .next()
    //         .unwrap();
    //     let (sdcard_addr, _) = find_device_addr(sdcard).unwrap().unwrap();
    //     let sdcard_base = unsafe { memory::map_device(sdcard_addr) }.as_ptr();
    //     println!("| SD Card controller addr: {:#010x}", sdcard_addr as usize);
    //     println!("| SD Card controller base: {:#010x}", sdcard_base as usize);
    //     let sdcard = unsafe { sdcard::bcm2711_emmc2_driver::init(sdcard_base) };
    //     unsafe { SD.init(SpinLock::new(sdcard)) };
    //     println!("| initialized SD Card");

    println!("| acquiring framebuffer");
    let mut surface = unsafe { MAILBOX.get().lock().get_framebuffer() };

    println!("| starting vsync demo; make sure to run with 'just run-ui'");
    vsync_tearing_demo(&mut surface);
}

fn vsync_tearing_demo(surface: &mut mailbox::Surface) {
    let (width, height) = surface.dimensions();

    for i in 0.. {
        let color = 0xFFFF0000 | (i as i32 % 512 - 256).abs().min(255) as u32;
        let color2 = 0xFF0000FF | (((i as i32 % 512 - 256).abs().min(255) as u32) << 16);
        let stripe_width = width / 20;
        let offset = i * (120 / surface.framerate());
        for r in 0..height {
            for c in 0..width {
                let cluster = (c + offset % (2 * stripe_width)) / stripe_width;
                let color = if cluster % 2 == 0 { color } else { color2 };
                surface[(r, c)] = color;
            }
        }

        surface.present();
        surface.wait_for_frame();
        // println!("SD Capacity: {}", SD.get().lock().get_capacity());
        // println!("SD Block Size: {}", SD.get().lock().get_block_size());
    }
}
