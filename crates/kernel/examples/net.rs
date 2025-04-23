#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use kernel::networking::repr::Ipv4Address;
#[allow(unused_imports)]
use kernel::networking::socket::{
    bind, connect, recv_from, send_to, SocketAddr, TcpSocket, UdpSocket,
};
use kernel::*;

#[no_mangle]
extern "Rust" fn kernel_main(_device_tree: device_tree::DeviceTree) {
    let count = 32;
    // WARN: this is unfortunately necessary to give dhcp time to resolve our ip address because
    // we don't have blocking currently
    for _i in 0..count {
        sync::spin_sleep(100_000);
    }

    // [udp send test]
    // let s = UdpSocket::new();
    // let saddr = SocketAddr {
    //     addr: Ipv4Address::new([11, 187, 10, 102]),
    //     port: 1337,
    // };
    // for _i in 0..5 {
    //     let _ = send_to(s, "hello everynyan".as_bytes().to_vec(), saddr);
    // }

    // [udp listening test]
    // To use this, send packets from your machine and uncomment this test
    //
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
    let saddr = SocketAddr {
        addr: Ipv4Address::new([11, 187, 10, 102]),
        port: 1337,
    };

    let s = TcpSocket::new();
    match connect(s, saddr) {
        Ok(_) => (),
        Err(_) => println!("couldn't connect"),
    };

    // WARN: same as above, but for tcp handshake
    for _i in 0..count {
        sync::spin_sleep(100_000);
    }

    for _i in 0..5 {
        let _ = send_to(s, "hello everynyan".as_bytes().to_vec(), saddr);
    }

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
}
