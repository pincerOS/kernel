use core::cell::RefCell;
use core::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};
use core::ops::Deref;
use core::hash::Hash;
use core::cmp::Ordering;
use alloc::rc::Rc;
use alloc::collections::btree_set::BTreeSet;

use crate::repr::Ipv4Address;
use crate::{ Error, Result };

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
// An IPv4 + port socket address.
pub struct SocketAddr {
    pub addr: Ipv4Address,
    pub port: u16,
}

impl Display for SocketAddr {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

// A socket address corresponding to different socket types.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TaggedSocketAddr {
    Udp(SocketAddr),
    // Tcp(SocketAddr),
}

// Custom implementation of PartialOrd for TaggedSocketAddr
impl PartialOrd for TaggedSocketAddr {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Custom implementation of Ord for TaggedSocketAddr
impl Ord for TaggedSocketAddr {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (TaggedSocketAddr::Udp(addr1), TaggedSocketAddr::Udp(addr2)) => {
                // First compare IP addresses as bytes
                let ip_cmp = addr1.addr.as_bytes().cmp(&addr2.addr.as_bytes());
                if ip_cmp != Ordering::Equal {
                    return ip_cmp;
                }
                // Then compare ports
                addr1.port.cmp(&addr2.port)
            },
            // For future extension with TCP support:
            // (TaggedSocketAddr::Tcp(addr1), TaggedSocketAddr::Tcp(addr2)) => {
            //     // First compare IP addresses as bytes
            //     let ip_cmp = addr1.addr.to_bytes().cmp(&addr2.addr.to_bytes());
            //     if ip_cmp != Ordering::Equal {
            //         return ip_cmp;
            //     }
            //     // Then compare ports
            //     addr1.port.cmp(&addr2.port)
            // },
            // // Udp comes before Tcp in ordering
            // (TaggedSocketAddr::Udp(_), TaggedSocketAddr::Tcp(_)) => Ordering::Less,
            // (TaggedSocketAddr::Tcp(_), TaggedSocketAddr::Udp(_)) => Ordering::Greater,
        }
    }
}

impl Deref for TaggedSocketAddr {
    type Target = SocketAddr;

    fn deref(&self) -> &SocketAddr {
        match *self {
            // TaggedSocketAddr::Tcp(ref addr) => addr,
            TaggedSocketAddr::Udp(ref addr) => addr,
        }
    }
}

impl Display for TaggedSocketAddr {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            // TaggedSocketAddr::Tcp(ref addr) => write!(f, "{} (TCP)", addr),
            TaggedSocketAddr::Udp(ref addr) => write!(f, "{} (UDP)", addr),
        }
    }
}

impl PartialEq<SocketAddr> for TaggedSocketAddr {
    fn eq(&self, socket_addr: &SocketAddr) -> bool {
        socket_addr == self.deref()
    }
}

// A socket address which has been reserved, and is freed for reallocation by
// the owning Bindings instance once dropped.
#[derive(Debug, Eq, PartialEq)]
pub struct SocketAddrLease {
    addr: TaggedSocketAddr,
    socket_addrs: Rc<RefCell<BTreeSet<TaggedSocketAddr>>>,
}

impl Deref for SocketAddrLease {
    type Target = TaggedSocketAddr;

    fn deref(&self) -> &TaggedSocketAddr {
        &self.addr
    }
}

impl Display for SocketAddrLease {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{} (Lease)", self.addr)
    }
}

impl Drop for SocketAddrLease {
    fn drop(&mut self) {
        self.socket_addrs.borrow_mut().remove(&self.addr);
    }
}

impl PartialEq<SocketAddr> for SocketAddrLease {
    fn eq(&self, socket_addr: &SocketAddr) -> bool {
        &self.addr == socket_addr
    }
}

// An allocator for socket address leases.
#[derive(Debug)]
pub struct Bindings {
    socket_addrs: Rc<RefCell<BTreeSet<TaggedSocketAddr>>>,
}

impl Bindings {
    // Creates a set of socket bindings.
    pub fn new() -> Bindings {
        Bindings {
            socket_addrs: Rc::new(RefCell::new(BTreeSet::new())),
        }
    }

    // Tries to reserve the specified UDP socket address, returning an
    // Error::InUse if the socket address is already in use.
    pub fn bind_udp(&self, socket_addr: SocketAddr) -> Result<SocketAddrLease> {
        self.bind(TaggedSocketAddr::Udp(socket_addr))
    }

    // Tries to reserve the specified TCP socket address, returning an
    // Error::InUse if the socket address is already in use.
    // pub fn bind_tcp(&self, socket_addr: SocketAddr) -> Result<SocketAddrLease> {
    //     self.bind(TaggedSocketAddr::Tcp(socket_addr))
    // }

    fn bind(&self, socket_addr: TaggedSocketAddr) -> Result<SocketAddrLease> {
        if self.socket_addrs.borrow_mut().insert(socket_addr.clone()) {
            Ok(SocketAddrLease {
                addr: socket_addr,
                socket_addrs: self.socket_addrs.clone(),
            })
        } else {
            Err(Error::BindingInUse(match socket_addr {
                TaggedSocketAddr::Udp(addr) => addr,
                // TaggedSocketAddr::Tcp(addr) => addr,
            }))
        }
    }
}

