#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use core::net::Ipv4Addr;

use kernel::{device::usb::device::net::get_dhcpd_mut, event::{task, thread}, networking::{iface::icmp, repr::{IcmpPacket, Ipv4Address}, socket::RawSocket}, ringbuffer};

#[allow(unused_imports)]
use kernel::networking::socket::{
    bind, accept, listen, connect, recv_from, send_to, SocketAddr, TcpSocket, UdpSocket,
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
    // println!("udp send test");
    // let s = UdpSocket::new();
    // let saddr = SocketAddr {
    //     addr: Ipv4Address::new([11, 187, 10, 102]),
    //     port: 1337,
    // };
    // for _i in 0..5 {
    //     let _ = send_to(s, "hello everynyan".as_bytes().to_vec(), saddr).await;
    // }
    // println!("end udp send test");


    // [udp listening test]
    // println!("udp listening test");
    // let s = UdpSocket::new();
    //
    // bind(s, 53);
    //
    // for i in 0..5 {
    //     println!("listening for packets");
    //     let recv = recv_from(s).await;
    //     if let Ok((payload, senderaddr)) = recv {
    //         println!("got message: {:x?}", payload);
    //     }
    // }
    //
    // println!("end udp listening test");


    // [tcp send test]
    // println!("tcp send test");
    // let saddr = SocketAddr {
    //     addr: Ipv4Address::new([11, 187, 10, 102]),
    //     port: 1337,
    // };
    //
    // let s = TcpSocket::new();
    // match connect(s, saddr).await {
    //     Ok(_) => (),
    //     Err(_) => println!("couldn't connect"),
    // };
    //
    // for _i in 0..5 {
    //     let _ = send_to(s, "hello everynyan".as_bytes().to_vec(), saddr);
    // }
    // println!("tcp send test end");


    // [tcp recv test]
    // let s = TcpSocket::new();
    //
    // bind(s, 1337);
    // listen(s, 1); // has a timeout, we will wait for 5 seconds
    //
    // let clientfd = accept(s).await;
    //
    // for i in 0..5 {
    //     let recv = recv_from(*clientfd.as_ref().unwrap()).await;
    //     if let Ok((payload, senderaddr)) = recv {
    //         println!("got message: {:x?}", payload);
    //     }
    // }

    // there is a delay when calling NetSend on a packet, this loop is to allow all the packets to
    // drain out
    for i in 0..32 {
        sync::spin_sleep(500_000);
    }


    shutdown();
}
