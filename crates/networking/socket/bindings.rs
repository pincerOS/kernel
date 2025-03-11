use std::cell::RefCell;
use std::collections::HashSet;
use std::net::SocketAddrV4;
use std::ops::Deref;
use std::rc::Rc;

use crate::repr::Ipv4Address;
use {Error, Result};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SocketAddr {
    pub addr: Ipv4Address,
    pub port: u16,
}

impl<'a> From<&'a SocketAddrV4> for SocketAddr {
    fn from(socket_addr: &'a SocketAddrV4) -> SocketAddr {
        SocketAddr {
            addr: Ipv4Address::from(socket_addr.ip()),
            port: socket_addr.port(),
        }
    }
}

impl Into<SocketAddrV4> for SocketAddr {
    fn into(self) -> SocketAddrV4 {
        SocketAddrV4::new(self.addr.into(), self.port)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TaggedSocketAddr {
    Udp(SocketAddr),
    // Tcp(SocketAddr),
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

impl PartialEq<SocketAddr> for TaggedSocketAddr {
    fn eq(&self, socket_addr: &SocketAddr) -> bool {
        socket_addr == self.deref()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SocketAddrLease {
    addr: TaggedSocketAddr,
    socket_addrs: Rc<RefCell<HashSet<TaggedSocketAddr>>>,
}

impl Deref for SocketAddrLease {
    type Target = TaggedSocketAddr;

    fn deref(&self) -> &TaggedSocketAddr {
        &self.addr
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
    socket_addrs: Rc<RefCell<HashSet<TaggedSocketAddr>>>,
}

impl Bindings {
    /// Creates a set of socket bindings.
    pub fn new() -> Bindings {
        Bindings {
            socket_addrs: Rc::new(RefCell::new(HashSet::new())),
        }
    }

    pub fn bind_udp(&self, socket_addr: SocketAddr) -> Result<SocketAddrLease> {
        self.bind(TaggedSocketAddr::Udp(socket_addr))
    }

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
