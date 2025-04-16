use crate::networking::iface::{icmp, ipv4, Interface};
use crate::networking::repr::*;
use crate::networking::socket::{SocketAddr, SocketSet, TaggedSocket};
use crate::networking::Result;

use alloc::vec::Vec;

use log::debug;

pub fn send_udp_packet(
    interface: &mut Interface,
    dst_addr: Ipv4Address,
    payload: Vec<u8>,
    src_port: u16,
    dst_port: u16,
) -> Result<()> {
    println!(
        "sending udp {} {} {} {}",
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
    let udp_packet = UdpPacket::deserialize(ipv4_packet.payload.as_slice())?;

    println!("received udp packet");

    let dst_socket_addr = SocketAddr {
        addr: ipv4_packet.dst_addr,
        port: udp_packet.dst_port,
    };
    let mut unreachable = true;

    interface
        .udp_sockets
        .iter_mut()
        .filter_map(|socket| match socket {
            TaggedSocket::Udp(ref mut socket) => {
                if socket.accepts(&dst_socket_addr) {
                    Some(socket)
                } else {
                    None
                }
            }
            _ => None,
        })
        .for_each(|socket| {
            unreachable = false;
            if let Err(err) = socket.recv_enqueue(&ipv4_packet, &udp_packet, &*udp_packet.payload) {
                debug!(
                    "Error enqueueing UDP packet for receiving via socket with {:?}.",
                    err
                );
            }
        });

    // Send an ICMP message indicating packet has been ignored because no
    // UDP sockets are bound to the specified port.
    if unreachable {
        let icmp_packet = IcmpPacket::new(IcmpMessage::DestinationUnreachable(
            IcmpDstUnreachable::PortUnreachable,
        ));

        ipv4::send_ipv4_packet(
            interface,
            icmp_packet.serialize(),
            Ipv4Protocol::ICMP,
            ipv4_packet.src_addr,
        )
    } else {
        Ok(())
    }
}
