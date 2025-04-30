use crate::networking::iface::{arp, ethernet, icmp, tcp, udp, Interface};
use crate::networking::repr::{EthernetFrame, EthernetType, Ipv4Address, Ipv4Packet, Ipv4Protocol};
use crate::networking::{Error, Result};

use crate::device::usb::device::net::{get_dhcpd_mut, get_interface_mut};
use crate::event::thread;
use crate::sync;

use alloc::vec::Vec;

pub fn send_ipv4_packet(
    interface: &mut Interface,
    payload: Vec<u8>,
    protocol: Ipv4Protocol,
    dst_addr: Ipv4Address,
) -> Result<()> {
    let next_hop = ipv4_addr_route(interface, dst_addr);
    match arp::eth_addr_for_ip(interface, next_hop) {
        Ok(dst_mac) => {
            println!("ip resolved: sending ip packet");

            let ipv4_packet = Ipv4Packet::new(*interface.ipv4_addr, dst_addr, protocol, payload);

            ethernet::send_ethernet_frame(
                interface,
                ipv4_packet.serialize(),
                dst_mac,
                EthernetType::IPV4,
            )
        }
        Err(e) => {
            println!("failed to resolve ip, queuing another send, waiting for ARP");
            thread::thread(move || {
                sync::spin_sleep(100_000);
                let interface = get_interface_mut();
                let _ = send_ipv4_packet(interface, payload, protocol, dst_addr);
            });
            Err(e)
        }
    }
}

pub fn recv_ip_packet(interface: &mut Interface, eth_frame: EthernetFrame) -> Result<()> {
    // println!("[!] received IP packet");
    let ipv4_packet = Ipv4Packet::deserialize(eth_frame.payload.as_slice())?;
    if !ipv4_packet.is_valid_checksum() {
        return Err(Error::Checksum);
    }

    let dhcpd = get_dhcpd_mut();

    if ipv4_packet.dst_addr != *interface.ipv4_addr
        && !interface.ipv4_addr.is_member(ipv4_packet.dst_addr)
        && !interface.ipv4_addr.is_broadcast(ipv4_packet.dst_addr)
        && !dhcpd.is_transacting()
    {
        return Err(Error::Ignored);
    }

    // update arp cache for immediate ICMP echo replies, errors, etc.
    if eth_frame.src.is_unicast() {
        let mut arp_cache = interface.arp_cache.lock();
        arp_cache.set_eth_addr_for_ip(ipv4_packet.src_addr, eth_frame.src);
    }

    match ipv4_packet.protocol {
        Ipv4Protocol::TCP => tcp::recv_tcp_packet(interface, ipv4_packet),
        Ipv4Protocol::UDP => udp::recv_udp_packet(interface, ipv4_packet),
        Ipv4Protocol::ICMP => icmp::recv_icmp_packet(interface, ipv4_packet),
        _ => Err(Error::Ignored),
    }
}

// get next hop for a packet destined to a specified address.
pub fn ipv4_addr_route(interface: &mut Interface, address: Ipv4Address) -> Ipv4Address {
    if interface.ipv4_addr.is_member(address) || interface.ipv4_addr.is_broadcast(address) {
        // println!("{} will be routed through link", address);
        address
    } else {
        println!("{} will be routed through default gateway", address);
        interface.default_gateway
    }
}
