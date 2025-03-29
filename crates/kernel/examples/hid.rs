#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use device::usb::mouse::{MouseEvent, MOUSE_EVENTS};
use kernel::device::usb::keyboard::Key;
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
    //Basic mouse & keyboard test
    let mut cur_x = 0;
    let mut cur_y = 0;

    loop {
        while let Some(event) = MOUSE_EVENTS.poll() {
            match event {
                MouseEvent::Move { x, y } => {
                    cur_x += x as i32;
                    cur_y += y as i32;
                    println!("| Mouse moved: x: {}, y: {}", cur_x, cur_y);
                }
                MouseEvent::Button {
                    button,
                    state,
                    all: _,
                } => {
                    if state {
                        println!("| Button pressed: {:?}", button);
                    } else {
                        println!("| Button released: {:?}", button);
                    }
                }
                MouseEvent::Wheel { delta } => {
                    println!("| Mouse wheel: {}", delta);
                }
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
        sleep(10000).await;
    }
}
