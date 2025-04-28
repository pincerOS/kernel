#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use core::net::Ipv4Addr;

use kernel::{device::usb::device::net::get_dhcpd_mut, event::{task, thread}, networking::{iface::icmp, repr::{HttpPacket, HttpMethod, IcmpPacket, Ipv4Address}, socket::RawSocket}, ringbuffer};

#[allow(unused_imports)]
use kernel::networking::socket::{
    bind, close, accept, listen, connect, recv_from, send_to, SocketAddr, TcpSocket, UdpSocket,
};
use kernel::networking::Result;
use kernel::*;

use alloc::string::String;

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

    // // [udp send test]
    // println!("udp send test");
    // let s = UdpSocket::new();
    // let saddr = SocketAddr {
    //     addr: Ipv4Address::new([10, 0, 2, 2]),
    //     port: 1337,
    // };
    // for _i in 0..100 {
    //     let _ = send_to(s, "hello everynyan\n".as_bytes().to_vec(), saddr).await;
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
    //     addr: Ipv4Address::new([10, 0, 2, 2]),
    //     port: 1337,
    // };
    //
    // let s = TcpSocket::new();
    // match connect(s, saddr).await {
    //     Ok(_) => (),
    //     Err(_) => println!("couldn't connect"),
    // };
    //
    // for _i in 0..100 {
    //     let _ = send_to(s, "hello everynyan\n".as_bytes().to_vec(), saddr).await;
    // }
    //
    // close(s).await;
    // println!("tcp send test end");


    // [tcp recv test]
    // let s = TcpSocket::new();
    //
    // bind(s, 22);
    // listen(s, 1).await;
    //
    // let clientfd = accept(s).await;
    //
    // let mut tot = 0;
    // while let recv = recv_from(*clientfd.as_ref().unwrap()).await {
    //     if let Ok((payload, senderaddr)) = recv {
    //         println!("got message: {:x?}", payload);
    //         tot += payload.len()
    //     } else {
    //         println!("\t[!] got a fin, ended");
    //         break;
    //     }
    // }
    //
    // println!("got {} bytes", tot);

    // [http request test]
    // println!("http send test");
    let host = "example.com";
    // let saddr = SocketAddr::resolve("host", 80);
    let saddr = SocketAddr {
        addr: Ipv4Address::new([23, 215, 0, 138]),
        port: 80,
    };

    let s = TcpSocket::new();
    match connect(s, saddr).await {
        Ok(_) => (),
        Err(_) => println!("couldn't connect"),
    };

    let path = "/";
    let http_req = HttpPacket::new(HttpMethod::Get, host, path);
    println!("{}",String::from_utf8(http_req.serialize()).unwrap());
    let _ = send_to(s, http_req.serialize(), saddr).await;

    close(s).await;
    println!("http send test end");
    

    shutdown();
}
