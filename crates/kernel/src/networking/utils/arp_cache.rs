use crate::device::system_timer;
use alloc::collections::BTreeMap;

use crate::networking::repr::{EthernetAddress, Ipv4Address};
// use crate::networking::utils::arp_cache::u64;
use core::time::Duration;

struct Entry {
    eth_addr: EthernetAddress,
    in_cache_since: u64,
}

// expiring set of IPv4 -> ethernet address mappings
pub struct ArpCache {
    entries: BTreeMap<Ipv4Address, Entry>,
    expiration: Duration,
    in_cache_since_min: u64,
}

impl ArpCache {
    pub fn new(expiration_in_secs: u64, now: u64) -> Self {
        ArpCache {
            entries: BTreeMap::new(),
            expiration: Duration::from_secs(expiration_in_secs),
            in_cache_since_min: now,
        }
    }

    pub fn eth_addr_for_ip(&mut self, ipv4_addr: Ipv4Address) -> Option<EthernetAddress> {
        self.expire_eth_addr();
        self.entries.get(&ipv4_addr).map(|entry| entry.eth_addr)
    }

    pub fn set_eth_addr_for_ip(&mut self, ipv4_addr: Ipv4Address, eth_addr: EthernetAddress) {
        self.expire_eth_addr();

        let now = system_timer::get_time(); // Use system_time::get_time() to get current time
        if self.entries.is_empty() {
            self.in_cache_since_min = now;
        }

        self.entries.insert(
            ipv4_addr,
            Entry {
                eth_addr,
                in_cache_since: now,
            },
        );
    }

    fn expire_eth_addr(&mut self) {
        let now = system_timer::get_time(); // Use system_time::get_time() to get current time

        // If the cache has been in use for longer than the expiration period
        if now > self.in_cache_since_min + (self.expiration.as_micros() as u64) {
            let expiration = self.expiration;
            self.entries.retain(|_, entry| {
                now.saturating_sub(entry.in_cache_since) <= (expiration.as_micros() as u64)
            });

            // Update the minimum in_cache_since timestamp
            let in_cache_since = self.entries.iter().map(|(_, entry)| entry.in_cache_since);
            self.in_cache_since_min = match in_cache_since.clone().min() {
                Some(min_time) => min_time,
                None => now,
            }
        }
    }
}
