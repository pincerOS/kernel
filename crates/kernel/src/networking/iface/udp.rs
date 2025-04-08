use crate::networking::repr::{
    Ipv4Packet,
    Ipv4Protocol,
    Ipv4Repr,
    UdpPacket,
    UdpRepr,
    IcmpMessage,
    IcmpRepr,
    IcmpDstUnreachable,
};
use crate::networking::iface::{icmp, ipv4, Interface};
use crate::networking::socket::{SocketAddr, SocketSet, TaggedSocket};
use crate::networking::{Result};

use log::debug;

pub fn send_packet<F>(
    interface: &mut Interface,
    ipv4_repr: &Ipv4Repr,
    udp_repr: &UdpRepr,
    f: F,
) -> Result<()>
where
    F: FnOnce(&mut [u8]),
{
    ipv4::send_packet_with_repr(interface, ipv4_repr, |ipv4_payload| {
        let mut udp_packet = UdpPacket::try_new(ipv4_payload).unwrap();
        f(udp_packet.payload_mut());
        udp_repr.serialize(&mut udp_packet, ipv4_repr);
    })
}

pub fn recv_packet(
    interface: &mut Interface,
    ipv4_repr: &Ipv4Repr,
    ipv4_packet: &Ipv4Packet<&[u8]>,
    socket_set: &mut SocketSet,
) -> Result<()> {
    let udp_packet = UdpPacket::try_new(ipv4_packet.payload())?;
    udp_packet.check_encoding(ipv4_repr)?;

    let udp_repr = UdpRepr::deserialize(&udp_packet);

    let dst_socket_addr = SocketAddr {
        addr: ipv4_repr.dst_addr,
        port: udp_repr.dst_port,
    };
    let mut unreachable = true;

    socket_set
        .iter_mut()
        .filter_map(|socket| match *socket {
            TaggedSocket::Udp(ref mut socket) => if socket.accepts(&dst_socket_addr) {
                Some(socket)
            } else {
                None
            },
            _ => None,
        })
        .for_each(|socket| {
            unreachable = false;
            if let Err(err) = socket.recv_enqueue(ipv4_repr, &udp_repr, udp_packet.payload()) {
                debug!(
                    "Error enqueueing UDP packet for receiving via socket with {:?}.",
                    err
                );
            }
        });

    // Send an ICMP message indicating packet has been ignored because no
    // UDP sockets are bound to the specified port.
    if unreachable {
        let icmp_repr = IcmpRepr {
            message: IcmpMessage::DestinationUnreachable(IcmpDstUnreachable::PortUnreachable),
            payload_len: 28, // IP header (20 bytes) + UDP header (8 bytes)
        };
        let ipv4_repr = Ipv4Repr {
            src_addr: *interface.ipv4_addr,
            dst_addr: ipv4_repr.src_addr,
            protocol: Ipv4Protocol::ICMP,
            payload_len: icmp_repr.buffer_len() as u16,
        };
        debug!(
            "Sending ICMP {:?} in response to a UDP {:?}.",
            icmp_repr, udp_repr
        );
        icmp::send_packet(interface, &ipv4_repr, &icmp_repr, |payload| {
            let copy_len = payload.len() as usize;
            payload.copy_from_slice(&ipv4_packet.as_ref()[.. copy_len]);
        })
    } else {
        Ok(())
    }
}
