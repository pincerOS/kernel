use crate::networking::iface::{arp, ipv4, Interface};
use crate::networking::repr::*;
use crate::networking::{Error, Result};

use crate::device::usb::device::net::NET_DEVICE;
use crate::device::usb::device::rndis::rndis_receive_packet;
use crate::event::thread;

use alloc::vec;
use alloc::vec::Vec;

// sends out an ethernet frame over an interface
pub fn send_ethernet_frame(
    interface: &mut Interface,
    payload: Vec<u8>,
    dst: EthernetAddress,
    ethtype: u16,
) -> Result<()> {
    let ethernet_packet = EthernetFrame {
        dst,
        src: interface.ethernet_addr,
        ethertype: ethtype,
        payload,
    };

    interface.dev.send(
        &mut ethernet_packet.serialize(),
        ethernet_packet.size() as u32,
    );
    Ok(())
}

// recv ethernet frame from interface: parsed -> fwd to socket -> propogated up stack
pub fn recv_ethernet_frame(interface: &mut Interface, eth_buffer: &[u8], len: u32) -> Result<()> {
    println!("[!] received ethernet frame");
    println!("\t {:x?}", eth_buffer);
    let eth_frame = EthernetFrame::deserialize(&eth_buffer[44..])?;

    // not for us
    // if eth_frame.dst != interface.ethernet_addr
    //     && !eth_frame.dst.is_broadcast()
    //     && !eth_frame.dst.is_multicast()
    // {
    //     return Err(Error::Ignored);
    // }

    let res = match eth_frame.ethertype {
        EthernetType::ARP => arp::recv_arp_packet(interface, eth_frame),
        EthernetType::IPV4 => ipv4::recv_ip_packet(interface, eth_frame),
        _ => Err(Error::Ignored),
    };

    thread::thread(move || {
        let buf = vec![0u8; 1500];
        unsafe {
            let mut_ref = &mut *NET_DEVICE.device.unwrap();
            rndis_receive_packet(mut_ref, buf.into_boxed_slice(), 1500);
        }
    });

    res
}
