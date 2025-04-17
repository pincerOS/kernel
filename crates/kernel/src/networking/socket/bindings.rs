use core::fmt::{Display, Formatter, Result as FmtResult};
use core::hash::Hash;

use alloc::collections::btree_set::BTreeSet;

use crate::networking::repr::Ipv4Address;

pub static mut BINDINGS: BTreeSet<u16> = BTreeSet::new();

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IpAddrPair {
    pub addr: Ipv4Address,
    pub port: u16,
}

impl IpAddrPair {
    pub fn default() -> Self {
        IpAddrPair {
            addr: Ipv4Address::new([0, 0, 0, 0]),
            port: 0,
        }
    }
}

impl Display for IpAddrPair {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}:{}", self.addr, self.port)
    }
}
