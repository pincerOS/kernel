#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::*;
use kernel::event::thread;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    // Basic preemption test
    let count = 32;
    let barrier = alloc::sync::Arc::new(sync::Barrier::new(count + 1));

    for i in 0..count {
        let b = barrier.clone();
        thread::thread(move || {
            println!("Starting thread {i}");
            sync::spin_sleep(500_000);
            println!("Ending thread {i}");
            b.sync();
        });
    }
    barrier.sync();
    println!("End of preemption test");
}
