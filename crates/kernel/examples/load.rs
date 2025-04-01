#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use event::task::yield_future;
use kernel::event::{task, thread};
use kernel::*;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| running load test");

    task::spawn_async(async move {
        for _ in 0..(1 << 10) {
            task::spawn_async(async move {
                loop {
                    yield_future().await;
                }
            });
        }
    });

    thread::stop();
}
