use crate::networking::iface::*;
use crate::networking::repr::*;
use crate::networking::socket::SockType;
use crate::networking::socket::SocketAddr;
use crate::networking::{Result, Error};

use crate::event::task;

use alloc::vec::Vec;

pub fn send_udp_packet(
    interface: &mut Interface,
    dst_addr: Ipv4Address,
    payload: Vec<u8>,
    src_port: u16,
    dst_port: u16,
) -> Result<()> {
    println!(
        "\t[!] sending udp {} {} {} {}",
        src_port, dst_port, *interface.ipv4_addr, dst_addr
    );

    let udp_packet = UdpPacket::new(src_port, dst_port, payload, *interface.ipv4_addr, dst_addr);

    ipv4::send_ipv4_packet(
        interface,
        udp_packet.serialize(),
        Ipv4Protocol::UDP,
        dst_addr,
    )
}

pub fn recv_udp_packet(interface: &mut Interface, ipv4_packet: Ipv4Packet) -> Result<()> {
    println!("\t received udp packet");
    let udp_packet = UdpPacket::deserialize(ipv4_packet.payload.as_slice())?;

    let local_socket_addr = SocketAddr {
        addr: ipv4_packet.dst_addr,
        port: udp_packet.dst_port,
    };

    let sender_socket_addr = SocketAddr {
        addr: ipv4_packet.src_addr,
        port: udp_packet.src_port,
    };

    // let mut sockets = interface.sockets.lock();
    
    for (_, socket) in interface.sockets.iter_mut() {
        if socket.binding_equals(local_socket_addr) {
            let (stype, mut tx) = socket.get_send_ref();

            if stype != SockType::UDP {
                return Err(Error::Unsupported);
            }

            let payload = udp_packet.payload.clone();

            task::spawn_async(async move {
                let _ = tx.send((payload, sender_socket_addr)).await;
            });
        }
    }

    Ok(())
}
