use alloc::collections::BTreeMap;
use core::time::Duration;

use crate::repr::{EthernetAddress, Ipv4Address};

// Replace std::time::Instant with a custom timestamp (e.g., u64 milliseconds)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Timestamp(u64);

struct Entry {
    eth_addr: EthernetAddress,
    in_cache_since: Timestamp,
}

// expiring set of IPv4 -> ethernet address mappings
pub struct ArpCache {
    entries: BTreeMap<Ipv4Address, Entry>,
    expiration: Duration,
    in_cache_since_min: Timestamp,
}

impl ArpCache {
    pub fn new(expiration_in_secs: u64, now: Timestamp) -> Self {
        ArpCache {
            entries: BTreeMap::new(),
            expiration: Duration::from_secs(expiration_in_secs),
            in_cache_since_min: now,
        }
    }

    pub fn eth_addr_for_ip(&mut self, ipv4_addr: Ipv4Address, now: Timestamp) -> Option<EthernetAddress> {
        self.expire_eth_addr(now);
        self.entries.get(&ipv4_addr).map(|entry| entry.eth_addr)
    }

    pub fn set_eth_addr_for_ip(&mut self, ipv4_addr: Ipv4Address, eth_addr: EthernetAddress, now: Timestamp) {
        self.expire_eth_addr(now);

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

    fn expire_eth_addr(&mut self, now: Timestamp) {
        if now.0 > self.in_cache_since_min.0 + self.expiration.as_secs() {
            let expiration_secs = self.expiration.as_secs();
            self.entries.retain(|_, entry| (now.0 - entry.in_cache_since.0) <= expiration_secs);

            self.in_cache_since_min = self.entries
                .values()
                .map(|entry| entry.in_cache_since)
                .min()
                .unwrap_or(now);
        }
    }
}
