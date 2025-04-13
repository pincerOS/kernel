use crate::networking::repr::{EthernetAddress, EthernetType, Ipv4Protocol, EthernetFrame, Ipv4Address, Ipv4Packet, Ipv4Repr};
use crate::networking::iface::{arp, ethernet, icmp, udp, Interface};
use crate::networking::{Error, Result};

use log::debug;

pub fn send_ipv4_packet(
    interface: &mut Interface, 
    payload: 
    dst_addr: Ipv4Address,
) -> Result<()> {
    let next_hop = ipv4_addr_route(interface, dst_addr);
    let dst_mac = interface.arp_cache.eth_addr_for_ip(next_hop);
    ethernet::send_frame(interface, ipv4_repr.serialize(), dst_mac, EthernetType::IPV4)
}

pub fn recv_ip_packet(
    interface: &mut Interface,
    eth_frame: EthernetFrame,
) -> Result<()> {
    let ipv4_packet = Ipv4Packet::deserialize(eth_frame.payload)?;
    if !ipv4_packet.is_valid_checksum() {
        return Err(Error::Checksum);
    }

    // TODO: broadcast
    if ipv4_packet.dst_addr != *interface.ipv4_addr {
        return Err(Error::Ignored);
    }

    // update arp cache for immediate ICMP echo replies, errors, etc.
    if eth_frame.src_addr().is_unicast() {
        interface
            .arp_cache
            .set_eth_addr_for_ip(ipv4_packet.src_addr, eth_frame.src_addr);
    }

    match ipv4_packet.protocol {
        Ipv4Protocol::TCP => tcp::recv_tcp_packet(interface, ipv4_packet.payload),
        Ipv4Protocol::UDP => udp::recv_udp_packet(interface, ipv4_packet.payload),
        Ipv4Protocol::ICMP => icmp::recv_icmp_packet(interface, ipv4_packet),
        i => {
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
