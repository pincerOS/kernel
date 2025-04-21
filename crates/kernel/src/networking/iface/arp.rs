use crate::networking::iface::{ethernet, Interface};
use crate::networking::repr::*;

use crate::networking::{Error, Result};

use log::debug;

pub fn send_arp_packet(
    interface: &mut Interface,
    arp_repr: &ArpPacket,
    dst_addr: EthernetAddress,
) -> Result<()> {
    ethernet::send_ethernet_frame(interface, arp_repr.serialize(), dst_addr, EthernetType::ARP)
}

pub fn recv_arp_packet(interface: &mut Interface, eth_frame: EthernetFrame) -> Result<()> {
    let arp_repr = ArpPacket::deserialize(eth_frame.payload.as_slice())?;

    if arp_repr.target_proto_addr != *interface.ipv4_addr {
        return Err(Error::Ignored);
    }

    println!(
        "\t[+] updating arp cache {} {}",
        arp_repr.source_proto_addr, arp_repr.source_hw_addr
    );

    interface
        .arp_cache
        .set_eth_addr_for_ip(arp_repr.source_proto_addr, arp_repr.source_hw_addr);

    match arp_repr.op {
        ArpOperation::Request => {
            let arp_reply = ArpPacket {
                op: ArpOperation::Reply,
                source_hw_addr: interface.ethernet_addr,
                source_proto_addr: *interface.ipv4_addr,
                target_hw_addr: arp_repr.source_hw_addr,
                target_proto_addr: arp_repr.source_proto_addr,
            };

            send_arp_packet(interface, &arp_reply, arp_reply.target_hw_addr)
        }
        _ => Ok(()),
    }
}

// matches mac -> ip
//      no mapping: arp req dispath + error
//      ip exists: processed by recv_packet -> update cache
pub fn eth_addr_for_ip(
    interface: &mut Interface,
    ipv4_addr: Ipv4Address,
) -> Result<EthernetAddress> {
    if interface.ipv4_addr.is_broadcast(ipv4_addr) {
        return Ok(EthernetAddress::BROADCAST);
    }

    match interface.arp_cache.eth_addr_for_ip(ipv4_addr) {
        Some(eth_addr) => Ok(eth_addr),
        None => {
            let arp_repr = ArpPacket {
                op: ArpOperation::Request,
                source_hw_addr: interface.ethernet_addr,
                source_proto_addr: *interface.ipv4_addr,
                target_hw_addr: EthernetAddress::BROADCAST,
                target_proto_addr: ipv4_addr,
            };

            debug!("Sending ARP request for {}.", ipv4_addr);
            send_arp_packet(interface, &arp_repr, EthernetAddress::BROADCAST)?;
            Err(Error::MacResolution(ipv4_addr))
        }
    }
}
