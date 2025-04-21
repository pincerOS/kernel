use alloc::boxed::Box;

pub mod arp;
pub mod cdcecm;
pub mod dhcp;
pub mod ethernet;
pub mod icmp;
pub mod ipv4;
pub mod socket;
pub mod tcp;
pub mod udp;

use crate::networking::repr::dev::Device;
use crate::networking::repr::{EthernetAddress, Ipv4Address, Ipv4Cidr};
use crate::networking::socket::TaggedSocket;
use crate::networking::utils::arp_cache::ArpCache;
use dhcp::DhcpClient;

use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;

// based this interface setup off of: https://github.com/ykskb/rust-user-net

pub struct Interface {
    pub dev: Box<dyn Device>, // device for sending and receiving raw ethernet frames
    pub arp_cache: ArpCache,
    pub ethernet_addr: EthernetAddress,

    pub sockets: BTreeMap<u16, TaggedSocket>,

    pub ipv4_addr: Ipv4Cidr,
    pub default_gateway: Ipv4Address,
    pub dns: Vec<Ipv4Address>,
    pub dhcp: DhcpClient,
}
