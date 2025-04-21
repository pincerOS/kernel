
use crate::networking::iface::*;
use crate::networking::repr::*;
use crate::networking::socket::{SocketAddr, TaggedSocket};
use crate::networking::Result;

use crate::device::usb::device::net::interface as get_interface;

use alloc::vec::Vec;

pub fn send_tcp_packet(
    interface: &mut Interface,
    dst_addr: Ipv4Address,
    payload: Vec<u8>,
    src_port: u16,
    dst_port: u16,
) -> Result<()> {
    println!(
        "\t[!] sending tcp {} {} {} {}",
        src_port, dst_port, *interface.ipv4_addr, dst_addr
    );

    let tcp_packet = TcpPacket::new(
        src_port, 
        dst_port, 
        payload, 
        *interface.ipv4_addr, 
        dst_addr
    );

    ipv4::send_ipv4_packet(
        interface,
        tcp_packet.serialize(),
        Ipv4Protocol::UDP,
        dst_addr,
    )
}

pub fn recv_tcp_packet(interface: &mut Interface, ipv4_packet: Ipv4Packet) -> Result<()> {
    println!("\t received tcp packet");
    let tcp_packet = TcpPacket::deserialize(ipv4_packet.payload.as_slice())?;

    let local_socket_addr = SocketAddr {
        addr: ipv4_packet.dst_addr,
        port: tcp_packet.dst_port,
    };

    let sender_socket_addr = SocketAddr {
        addr: ipv4_packet.src_addr,
        port: tcp_packet.src_port,
    };

    for (_, socket) in &mut interface.sockets {
        if socket.binding_equals(local_socket_addr) {
            socket.recv_enqueue(tcp_packet.payload.clone(), sender_socket_addr);
        }
    }

    // TODO: dns

    Ok(())
}
