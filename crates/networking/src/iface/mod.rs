pub mod arp;
pub mod ethernet;
pub mod icmp;
pub mod ipv4;
pub mod socket;
// pub mod tcp;
pub mod udp;

use crate::utils::arp_cache::ArpCache;
use crate::repr::{EthernetAddress, Ipv4Address, Ipv4Cidr};
use crate::repr::dev::Device;

// based this interface setup off of: https://github.com/ykskb/rust-user-net

// HACK: idk how to solve compile time device diff without generics, foudn this on stack exchange
pub struct Interface {
    pub dev: Box<dyn Device>,   // device for sending and receiving raw ethernet frames
    pub arp_cache: ArpCache,
    pub ethernet_addr: EthernetAddress,
    pub ipv4_addr: Ipv4Cidr,
    pub default_gateway: Ipv4Address,
}
