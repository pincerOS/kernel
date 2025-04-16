use core::fmt::{Display, Formatter, Result as FmtResult};
use core::hash::Hash;

use crate::networking::repr::Ipv4Address;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SocketAddr {
    pub addr: Ipv4Address,
    pub port: u16,
}

impl SocketAddr {
    pub fn default() -> Self {
        SocketAddr {
            addr: Ipv4Address::new([0, 0, 0, 0]),
            port: 0
        }
    }
}

impl Display for SocketAddr {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

