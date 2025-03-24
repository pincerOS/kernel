#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::device::system_timer::micro_delay;
use kernel::device::usb::keyboard::Key;
use kernel::*;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    //Basic mouse & keyboard test
    let mut cur_x = 0;
    let mut cur_y = 0;

    loop {
        let mouse = device::usb::usb_retrieve_mouse();
        if mouse.x != 0 || mouse.y != 0 || mouse.buttons != 0 || mouse.wheel != 0 {
            cur_x += mouse.x as i32;
            cur_y += mouse.y as i32;

            if mouse.x != 0 || mouse.y != 0 {
                println!("| Mouse moved: x: {}, y: {}", cur_x, cur_y);
            }

            if mouse.buttons != 0 {
                print!("| Mouse buttons: ");
                if mouse.buttons & 0x01 != 0 {
                    print!("Left ");
                }
                if mouse.buttons & 0x02 != 0 {
                    print!("Right ");
                }
                if mouse.buttons & 0x04 != 0 {
                    print!("Middle ");
                }
                if mouse.buttons & 0x08 != 0 {
                    print!("Mouse-5 ");
                }
                if mouse.buttons & 0x10 != 0 {
                    print!("Mouse-4 ");
                }
                println!();
            }

            if mouse.wheel != 0 {
                println!("| Mouse wheel: {}", mouse.wheel);
            }
        }

        while let Some(event) = device::usb::keyboard::KEY_EVENTS.poll() {
            if event.pressed {
                if event.key == Key::Return {
                    println!();
                } else {
                    print!("{:?} ", event.key);
                }
            }
        }
        micro_delay(10000);
    }
}
