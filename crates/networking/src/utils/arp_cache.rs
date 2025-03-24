use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::repr::{EthernetAddress, Ipv4Address};

struct Entry {
    eth_addr: EthernetAddress,
    in_cache_since: Instant,
}

// Maintains an expiring set of IPv4 -> ethernet address mappings
pub struct ArpCache {
    entries: HashMap<Ipv4Address, Entry>,
    expiration: Duration,
    in_cache_since_min: Instant,
}

impl ArpCache {
    // Creates an ARP cache where ethernet address mappings expire after
    // expiration_in_secs seconds
    pub fn new(expiration_in_secs: u64) -> Self {
        ArpCache {
            entries: HashMap::new(),
            expiration: Duration::from_secs(expiration_in_secs),
            in_cache_since_min: Instant::now(),
        }
    }

    // Lookup the ethernet address for an IPv4 address
    pub fn eth_addr_for_ip(&mut self, ipv4_addr: Ipv4Address) -> Option<EthernetAddress> {
        self.expire_eth_addr();

        self.entries.get(&ipv4_addr).map(|entry| entry.eth_addr)
    }

    // Create or update the ethernet address mapping for an IPv4 address
    pub fn set_eth_addr_for_ip(&mut self, ipv4_addr: Ipv4Address, eth_addr: EthernetAddress) {
        self.expire_eth_addr();

        let in_cache_since = Instant::now();

        if self.entries.is_empty() {
            self.in_cache_since_min = in_cache_since;
        }

        self.entries.insert(
            ipv4_addr,
            Entry {
                eth_addr,
                in_cache_since,
            },
        );
    }

    // Purge Ethernet address entries translations that have expired
    fn expire_eth_addr(&mut self) {
        let now = Instant::now();

        if now > self.in_cache_since_min + self.expiration {
            // Purge expired entries
            let expiration = self.expiration;
            self.entries
                .retain(|_, entry| now.duration_since(entry.in_cache_since) <= expiration);

            // Update timestamp of the oldest entry
            self.in_cache_since_min = self.entries
                .values()
                .map(|entry| entry.in_cache_since)
                .min()
                .unwrap_or(now);
        }
    }
}
