#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use alloc::sync::Arc;
use event::task::spawn_async;
use kernel::*;
use sync::time::interval;
use sync::Barrier;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");
    crate::event::task::spawn_async(async move {
        main().await;
    });
    crate::event::thread::stop();
}

async fn main() {
    let total_time = 5000;
    let count = 64;
    let barrier = Arc::new(Barrier::new(count + 1));

    for i in 0..count as u64 {
        let b: Arc<Barrier> = barrier.clone();
        spawn_async(async move {
            let period = (total_time / (i + 1)).max(1);
            let steps = total_time / period;
            let mut interval = interval(period * 1000);
            println!("Task {i} started with period of {period}");
            for _ in 0..steps {
                interval.tick().await;
            }
            println!("Task {i} done");
            b.sync().await;
        });
    }

    barrier.sync().await;
    println!("End of async sleep scheduling test");
    shutdown();
}
