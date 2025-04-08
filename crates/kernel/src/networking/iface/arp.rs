use crate::networking::repr::{EthernetType, ArpPacket, ArpOperation, EthernetAddress, EthernetFrame, Ipv4Address};
use crate::networking::iface::{ethernet,Interface};

use crate::networking::{Error,Result};

use log::debug;

pub fn send_packet(
    interface: &mut Interface,
    arp_repr: &ArpPacket,
    dst_addr: EthernetAddress,
) -> Result<()> {
    let eth_frame_len = EthernetFrame::<&[u8]>::buffer_len(arp_repr.buffer_len());

    ethernet::send_frame(interface, eth_frame_len, |eth_frame| {
        eth_frame.set_dst_addr(dst_addr);
        eth_frame.set_payload_type(EthernetType::ARP);
        arp_repr.serialize(eth_frame.payload_mut()).unwrap();
    })
}

pub fn recv_packet(interface: &mut Interface, eth_frame: &EthernetFrame<&[u8]>) -> Result<()> {
    let arp_repr = ArpPacket::deserialize(eth_frame.payload())?;
    if arp_repr.target_proto_addr != *interface.ipv4_addr {
        debug!(
            "Ignoring ARP with target IPv4 address {}.",
            arp_repr.target_proto_addr
        );
        return Err(Error::Ignored);
    }

    debug!(
        "Received ARP, adding mapping from {} to {}.",
        arp_repr.source_proto_addr, arp_repr.source_hw_addr
    );
    interface
        .arp_cache
        .set_eth_addr_for_ip(arp_repr.source_proto_addr, arp_repr.source_hw_addr);

    match arp_repr.op {
        ArpOperation::Request => {
            let arp_reply = ArpPacket{
                op: ArpOperation::Reply,
                source_hw_addr: interface.ethernet_addr,
                source_proto_addr: *interface.ipv4_addr,
                target_hw_addr: arp_repr.source_hw_addr,
                target_proto_addr: arp_repr.source_proto_addr,
            };

            debug!(
                "Sending ARP reply to {}/{}.",
                arp_reply.target_proto_addr, arp_reply.target_hw_addr
            );

            send_packet(interface, &arp_reply, arp_reply.target_hw_addr)
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
            send_packet(interface, &arp_repr, EthernetAddress::BROADCAST)?;
            Err(Error::MacResolution(ipv4_addr))
        }
    }
}
