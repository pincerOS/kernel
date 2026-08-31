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

    println!("| acquiring framebuffer");
    let mut surface = unsafe { mailbox.get_framebuffer() };

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
    }
}
