#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use device::mailbox;
use kernel::{device::MAILBOX, *};

#[no_mangle]
extern "Rust" fn kernel_main(tree: device_tree::DeviceTree<'static>) {
    println!("| starting kernel_main");
    crate::event::task::spawn_async(async move {
        main(tree).await;
    });
    crate::event::thread::stop();
}

async fn main(_tree: device_tree::DeviceTree<'static>) {
    println!("| acquiring framebuffer");
    let mut surface = unsafe { MAILBOX.get().lock().map_framebuffer_kernel(640, 480) };

    println!("| starting vsync demo; make sure to run with 'just run-ui'");
    vsync_tearing_demo(&mut surface).await;
}

async fn vsync_tearing_demo(surface: &mut mailbox::Surface) {
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
        surface.wait_for_frame().await;
        // println!("SD Capacity: {}", SD.get().lock().get_capacity());
        // println!("SD Block Size: {}", SD.get().lock().get_block_size());
    }
}
