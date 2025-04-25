use crate::networking::iface::{ethernet, Interface};
use crate::networking::repr::{
    ArpOperation, ArpPacket, EthernetAddress, EthernetFrame, EthernetType, Ipv4Address,
};
use crate::networking::{Error, Result};

pub fn send_arp_packet(
    interface: &mut Interface,
    op: ArpOperation,
    dst_addr: EthernetAddress,
    target_hw_addr: EthernetAddress,
    target_proto_addr: Ipv4Address,
) -> Result<()> {
    let arp_repr = ArpPacket {
        op,
        source_hw_addr: interface.ethernet_addr,
        source_proto_addr: *interface.ipv4_addr,
        target_hw_addr,
        target_proto_addr,
    };

    ethernet::send_ethernet_frame(interface, arp_repr.serialize(), dst_addr, EthernetType::ARP)
}

pub fn recv_arp_packet(interface: &mut Interface, eth_frame: EthernetFrame) -> Result<()> {
    println!("[!] received arp packet");
    let arp_repr = ArpPacket::deserialize(eth_frame.payload.as_slice())?;

    println!(
        "\tupdating arp cache {}/{}",
        arp_repr.source_proto_addr, arp_repr.source_hw_addr
    );

    // if the target_protocol address isn't us, we'll ignore it for now. be selfish we don't give
    // out other people's numbers :(
    if arp_repr.target_proto_addr != *interface.ipv4_addr {
        return Err(Error::Ignored);
    }

    // update the arp cache with the information of the sender
    let mut arp_cache = interface.arp_cache.lock();
    arp_cache.set_eth_addr_for_ip(arp_repr.source_proto_addr, arp_repr.source_hw_addr);
    drop(arp_cache);

    match arp_repr.op {
        ArpOperation::Request => send_arp_packet(
            interface,
            ArpOperation::Reply,
            arp_repr.source_hw_addr,
            arp_repr.source_hw_addr,
            arp_repr.source_proto_addr,
        ),
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

    let mut arp_cache = interface.arp_cache.lock();
    let eth_addr = arp_cache.eth_addr_for_ip(ipv4_addr);
    drop(arp_cache);

    match eth_addr {
        Some(eth_addr) => Ok(eth_addr),
        None => {
            println!("address not found, sending ARP request for {}", ipv4_addr);
            send_arp_packet(
                interface,
                ArpOperation::Request,
                EthernetAddress::BROADCAST,
                EthernetAddress::BROADCAST,
                ipv4_addr,
            )?;
            Err(Error::MacResolution(ipv4_addr))
        }
    }
}
