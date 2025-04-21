#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::event::thread;
use kernel::networking::repr::Ipv4Address;
use kernel::networking::socket::{
    bind, connect, recv_from, send_to, SocketAddr, TcpSocket, UdpSocket,
};
use kernel::*;

use core::str;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");

    // Basic preemption test
    let count = 32;
    let barrier = alloc::sync::Arc::new(sync::Barrier::new(count + 1));

    // for i in 0..count {
    //     let b = barrier.clone();
    //     thread::thread(move || {
    //         println!("Starting thread {i}");
    //         sync::spin_sleep(500_000);
    //         println!("Ending thread {i}");
    //         b.sync_blocking();
    //     });
    // }
    // barrier.sync_blocking();
    // println!("End of preemption test");

    // [udp send test]
    // let s = UdpSocket::new();
    // let saddr = SocketAddr {
    //     addr: Ipv4Address::new([11, 187, 10, 102]),
    //     port: 2222,
    // };
    // send_to(s, "hello everynyan".as_bytes().to_vec(), saddr);

    // [udp listening test]
    // let s = UdpSocket::new();
    //
    // bind(s, 2222);
    //
    // loop {
    //     sync::spin_sleep(500_000);
    //     let recv = recv_from(s);
    //     if let Ok((payload, senderaddr)) = recv {
    //         println!("got message: {:x?}", payload);
    //     }
    // }

    // [tcp send test]
    // for i in 0..count {
    //     sync::spin_sleep(100_000);
    // }
    // let saddr = SocketAddr {
    //     addr: Ipv4Address::new([11, 187, 10, 102]),
    //     port: 2222,
    // };
    //
    // let s = TcpSocket::new();
    // connect(s, saddr);
    // // TODO: connect, recv, and dhcp init really need blocking
    // for i in 0..count {
    //     sync::spin_sleep(100_000);
    // }
    // send_to(s, "hello everynyan".as_bytes().to_vec(), saddr);

    // [tcp recv test]
    // let s = TcpSocket::new();
    //
    // bind(s, 2222);
    // listen(s);
    //
    // let client = accept(s);
    //
    // recv_from(s, client);
}
