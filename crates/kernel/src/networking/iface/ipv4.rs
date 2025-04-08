use crate::networking::repr::{EthernetType, Ipv4Protocol, EthernetFrame, Ipv4Address, Ipv4Packet, Ipv4Repr};
use crate::networking::iface::{arp, ethernet, icmp, udp, Interface};
use crate::networking::socket::{RawType, SocketSet, TaggedSocket};
use crate::networking::{Error, Result};

use log::debug;

pub fn send_packet_raw<F>(
    interface: &mut Interface,
    dst_addr: Ipv4Address,
    ipv4_packet_len: usize,
    f: F,
) -> Result<()>
where
    F: FnOnce(&mut [u8]),
{
    let dst_addr = ipv4_addr_route(interface, dst_addr);
    let eth_dst_addr = arp::eth_addr_for_ip(interface, dst_addr)?;
    let eth_frame_len = EthernetFrame::<&[u8]>::buffer_len(ipv4_packet_len);

    ethernet::send_frame(interface, eth_frame_len, |eth_frame| {
        eth_frame.set_dst_addr(eth_dst_addr);
        eth_frame.set_payload_type(EthernetType::IPV4);
        f(eth_frame.payload_mut());
    })
}

// buffer only 
pub fn send_packet_with_repr<F>(interface: &mut Interface, ipv4_repr: &Ipv4Repr, f: F) -> Result<()>
where
    F: FnOnce(&mut [u8]),
{
    let (dst_addr, ipv4_packet_len) = (ipv4_repr.dst_addr, ipv4_repr.buffer_len());

    send_packet_raw(interface, dst_addr, ipv4_packet_len, |ipv4_buffer| {
        let mut ipv4_packet = Ipv4Packet::try_new(ipv4_buffer).unwrap();
        ipv4_repr.serialize(&mut ipv4_packet);
        f(ipv4_packet.payload_mut());
    })
}

pub fn recv_packet(
    interface: &mut Interface,
    eth_frame: &EthernetFrame<&[u8]>,
    socket_set: &mut SocketSet,
) -> Result<()> {
    let ipv4_packet = Ipv4Packet::try_new(eth_frame.payload())?;
    ipv4_packet.check_encoding()?;

    if ipv4_packet.dst_addr() != *interface.ipv4_addr {
        debug!(
            "Ignoring IPv4 packet with destination {}.",
            ipv4_packet.dst_addr()
        );
        return Err(Error::Ignored);
    }

    // update arp cache for immediate ICMP echo replies, errors, etc.
    if eth_frame.src_addr().is_unicast() {
        interface
            .arp_cache
            .set_eth_addr_for_ip(ipv4_packet.src_addr(), eth_frame.src_addr());
    }

    socket_set
        .iter_mut()
        .filter_map(|socket| match *socket {
            TaggedSocket::Raw(ref mut socket) => if socket.raw_type() == RawType::Ipv4 {
                Some(socket)
            } else {
                None
            },
            _ => None,
        })
        .for_each(|socket| {
            if let Err(err) = socket.recv_enqueue(ipv4_packet.as_ref()) {
                debug!(
                    "Error enqueueing IPv4 packet for receiving via socket with {:?}.",
                    err
                );
            }
        });

    let ipv4_repr = Ipv4Repr::deserialize(&ipv4_packet)?;

    // match ipv4_packet.protocol() {
    //     // Ipv4Protocol::TCP => tcp::recv_packet(interface, &ipv4_repr, &ipv4_packet, socket_set),
    //     Ipv4Protocol::UDP => udp::recv_packet(interface, &ipv4_repr, &ipv4_packet, socket_set),
    //     Ipv4Protocol::ICMP => icmp::recv_packet(interface, &ipv4_repr, ipv4_packet.payload()),
    //     i => {
    //         debug!("Ignoring IPv4 packet with type {}.", i);
    //         Err(Error::Ignored)
    //     }
    // }
    match ipv4_packet.protocol() {
        x if x == Ipv4Protocol::UDP as u8 => udp::recv_packet(interface, &ipv4_repr, &ipv4_packet, socket_set),
        x if x == Ipv4Protocol::ICMP as u8 => icmp::recv_packet(interface, &ipv4_repr, ipv4_packet.payload()),
        i => {
            debug!("Ignoring IPv4 packet with type {}.", i);
            Err(Error::Ignored)
    }
}

}

// get next hop for a packet destined to a specified address.
pub fn ipv4_addr_route(interface: &mut Interface, address: Ipv4Address) -> Ipv4Address {
    if interface.ipv4_addr.is_member(address) {
        debug!("{} will be routed through link.", address);
        address
    } else {
        debug!("{} will be routed through default gateway.", address);
        interface.default_gateway
    }
}
