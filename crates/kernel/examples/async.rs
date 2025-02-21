#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::event::{task, thread};
use kernel::*;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    task::spawn_async(async move {
        let count = 32;
        let barrier = alloc::sync::Arc::new(sync::Barrier::new(count + 1));

        for i in 0..count {
            let b: alloc::sync::Arc<sync::Barrier> = barrier.clone();
            task::spawn_async(async move {
                println!("Starting thread {i}");
                // TODO: non-spinning sleep
                sync::spin_sleep(500_000);
                println!("Ending thread {i}");
                b.sync_async().await;
            });
        }
        barrier.sync_async().await;
        println!("End of preemption test");
        shutdown();
    });

    thread::stop();
}
