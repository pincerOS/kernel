#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::event::thread;
use kernel::networking::repr::Ipv4Address;
use kernel::networking::socket::{bind, recv_from, send_to, SocketAddr, UdpSocket};
use kernel::*;

use core::str;

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
            b.sync_blocking();
        });
    }
    barrier.sync_blocking();
    println!("End of preemption test");

    let s = UdpSocket::new();

    // send_to(s, "hello everynyan".as_bytes().to_vec(), saddr);
    bind(s, 2222);

    // for i in 0..count {
    //     sync::spin_sleep(5000_000);
    // }

    loop {
        sync::spin_sleep(500_000);
        let recv = recv_from(s);
        if let Ok((payload, senderaddr)) = recv {
            println!("got message: {:x?}", payload);
        }
    }
}
