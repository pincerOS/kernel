#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use alloc::sync::Arc;
use event::schedule;
use kernel::*;
use sync::Barrier;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");
    schedule(move || {
        println!("| running first task");

        let count = 8;
        let barrier = Arc::new(sync::Barrier::new(count + 1));

        for i in 0..count {
            let b = barrier.clone();
            schedule(move || {
                println!("Starting thread {i}");
                Barrier::sync_then(b, move || {
                    println!("After barrier {i}");
                });
            });
        }
        Barrier::sync_then(barrier, move || {
            println!("End of barrier test");

            sync::spin_sleep(500_000);

            kernel::shutdown();
        });
    });
    thread::stop();
}
