use crate::networking::iface::{ipv4, Interface};
use crate::networking::repr::*;
use crate::networking::socket::{SocketAddr, TaggedSocket};
use crate::networking::{Error, Result};
use alloc::vec::Vec;

// Send a TCP packet through the interface
pub fn send_tcp_packet(
    interface: &mut Interface,
    dst_addr: Ipv4Address,
    payload: Vec<u8>,
    src_port: u16,
    dst_port: u16,
    seq_number: u32,
    ack_number: u32,
    flags: u8,
    window_size: u16,
) -> Result<()> {
    println!(
        "sending tcp {} {} {} {} seq={} ack={} flags={:#04x}",
        src_port, dst_port, *interface.ipv4_addr, dst_addr, seq_number, ack_number, flags
    );

    let tcp_packet = TcpPacket::new(
        src_port,
        dst_port,
        seq_number,
        ack_number,
        flags,
        window_size,
        payload,
        *interface.ipv4_addr,
        dst_addr,
    );

    ipv4::send_ipv4_packet(
        interface,
        tcp_packet.serialize(),
        Ipv4Protocol::TCP,
        dst_addr,
    )
}

// Process an incoming TCP packet
pub fn recv_tcp_packet(interface: &mut Interface, ipv4_packet: Ipv4Packet) -> Result<()> {
    let tcp_packet = TcpPacket::deserialize(ipv4_packet.payload.as_slice())?;

    // Set source and destination IP addresses for validation and checksum verification
    let mut packet_with_ips = tcp_packet.clone();
    packet_with_ips.src_ip = ipv4_packet.src_addr;
    packet_with_ips.dst_ip = ipv4_packet.dst_addr;

    // Socket address of the destination
    let dst_socket_addr = SocketAddr {
        addr: ipv4_packet.dst_addr,
        port: tcp_packet.dst_port,
    };

    // Socket address of the source
    let src_socket_addr = SocketAddr {
        addr: ipv4_packet.src_addr,
        port: tcp_packet.src_port,
    };

    let mut handled = false;

    // Find and process any sockets that should receive this packet
    // interface.tcp_sockets
    //     .iter_mut()
    //     .filter_map(|socket| match socket {
    //         TaggedSocket::Tcp(ref mut socket) => {
    //             if socket.accepts(&dst_socket_addr) {
    //                 Some(socket)
    //             } else {
    //                 None
    //             }
    //         }
    //         _ => None,
    //     })
    //     .for_each(|socket| {
    //         handled = true;
    //         if let Err(err) = socket.process_packet(&ipv4_packet, &packet_with_ips) {
    //             println!(
    //                 "Error processing TCP packet for socket: {:?}",
    //                 err
    //             );
    //         }
    //     });
    //
    // // If no socket handled the packet, check if we need to send a reset
    // if !handled {
    //     // If the packet isn't a reset itself, send a reset
    //     if (tcp_packet.flags & TCP_RST) == 0 {
    //         return send_tcp_reset(interface, &packet_with_ips, ipv4_packet.src_addr);
    //     }
    // }

    Ok(())
}
