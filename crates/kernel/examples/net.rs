#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::{ringbuffer, event::{task, thread}, device::usb::device::net::get_dhcpd_mut, networking::repr::Ipv4Address};

#[allow(unused_imports)]
use kernel::networking::socket::{
    bind, connect, recv_from, send_to, SocketAddr, TcpSocket, UdpSocket,
};
use kernel::networking::Result;
use kernel::*;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    println!("| starting kernel_main");
    task::spawn_async(async move {
        main().await;
    });
    crate::event::thread::stop();
}

async fn main() {
    println!("starting dhcpd");

    let dhcpd = get_dhcpd_mut();
    dhcpd.start().await;

    println!("out of dhcpd");

    // [udp send test]
    println!("udp send test");
    let s = UdpSocket::new();
    let saddr = SocketAddr {
        addr: Ipv4Address::new([11, 187, 10, 102]),
        port: 1337,
    };
    for _i in 0..5 {
        let _ = send_to(s, "hello everynyan".as_bytes().to_vec(), saddr).await;
    }
    println!("end udp send test");


    // [udp listening test]
    // let s = UdpSocket::new();
    //
    // bind(s, 2222);
    //
    // loop {
    //     let recv = recv_from(s).await;
    //     if let Ok((payload, senderaddr)) = recv {
    //         println!("got message: {:x?}", payload);
    //     }
    // }

    // [tcp send test]
    // println!("tcp send test");
    // let saddr = SocketAddr {
    //     addr: Ipv4Address::new([11, 187, 10, 102]),
    //     port: 1337,
    // };
    //
    // let s = TcpSocket::new();
    // match connect(s, saddr) {
    //     Ok(_) => (),
    //     Err(_) => println!("couldn't connect"),
    // };
    //
    // for _i in 0..5 {
    //     let _ = send_to(s, "hello everynyan".as_bytes().to_vec(), saddr);
    // }
    // println!("tcp send test end");

    // [tcp recv test]
    // To use this, send packets from your machine and uncomment this test

    // let s = TcpSocket::new();
    //
    // bind(s, 2222);
    // listen(s); // has a timeout, we will wait for 5 seconds
    //
    // let client = accept(s);
    // // WARN: same as above, but for tcp handshake
    // for _i in 0..count {
    //     sync::spin_sleep(100_000);
    // }
    //
    // loop {
    //     sync::spin_sleep(500_000);
    //     let recv = recv_from(s);
    //     if let Ok((payload, senderaddr)) = recv {
    //         println!("got message: {:x?}", payload);
    //     }
    // }
    shutdown();
}
