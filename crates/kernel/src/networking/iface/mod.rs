/** [`iface`] module
* based this interface setup off of: https://github.com/ykskb/rust-user-net
* the Interface will store all of our shared values, including
*   1. arp cache
*   2. addresses
*   3. device (where we actually send and recv our packets)
*   4. sockets (see the [`socket`] module for more)
*/
use crate::device::system_timer;
use crate::sync::SpinLock;

use crate::networking::repr::{Device, EthernetAddress, Ipv4Address, Ipv4Cidr};
use crate::networking::socket::TaggedSocket;
use crate::networking::utils::arp_cache::ArpCache;

use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;

pub mod arp;
pub mod cdcecm;
pub mod dhcp;
pub mod ethernet;
pub mod icmp;
pub mod ipv4;
pub mod socket;
pub mod tcp;
pub mod udp;

use cdcecm::CDCECM;

// WARN: for now, we assume that we will only ever set our ethernet_addr, ipv4_addr, and
// default_gateway once when we first initialize
pub struct Interface {
    pub dev: Box<dyn Device>,

    pub arp_cache: SpinLock<ArpCache>,
    pub ethernet_addr: EthernetAddress,

    pub ipv4_addr: Ipv4Cidr,
    pub default_gateway: Ipv4Address,

    pub sockets: SpinLock<BTreeMap<u16, TaggedSocket>>,
}

impl Interface {
    pub fn new() -> Self {
        Interface {
            dev: Box::new(CDCECM::new(1500)),
            arp_cache: SpinLock::new(ArpCache::new(60, system_timer::get_time())),
            ethernet_addr: EthernetAddress::empty(),
            ipv4_addr: Ipv4Cidr::empty(),
            default_gateway: Ipv4Address::empty(),

            sockets: SpinLock::new(BTreeMap::new()),
        }
    }
}
