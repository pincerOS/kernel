use core::fmt::{Display, Formatter, Result as FmtResult};
use core::hash::Hash;
use core::sync::atomic::{AtomicU16, Ordering};

use alloc::collections::btree_set::BTreeSet;
use alloc::vec::Vec;

use crate::device::usb::device::net::interface;
use crate::networking::repr::Ipv4Address;
use crate::networking::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SocketAddr {
    pub addr: Ipv4Address,
    pub port: u16,
}

impl SocketAddr {
    pub fn default() -> Self {
        SocketAddr {
            addr: Ipv4Address::new([0, 0, 0, 0]),
            port: 0,
        }
    }
}

impl Display for SocketAddr {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

// TODO: these technically runs out eventually lol need wrap around
pub static NEXT_EPHEMERAL: AtomicU16 = AtomicU16::new(32768);
pub static NEXT_SOCKETFD: AtomicU16 = AtomicU16::new(1);

pub fn send_to(socketfd: u16, payload: Vec<u8>, saddr: SocketAddr) -> Result<()> {
    let sockets = &mut interface().sockets;

    // 1. check if socket fd is valid if not return error
    let tagged_socket = sockets
        .get_mut(&socketfd)
        .ok_or(Error::InvalidSocket(socketfd))?;

    // 2. if socket not bound, bind to ephemeral port (32768–60999)
    if !tagged_socket.is_bound() {
        let ephem_port = NEXT_EPHEMERAL.fetch_add(1, Ordering::SeqCst);

        tagged_socket.bind(ephem_port);
    }

    // 3. queue a send on socket sending queue
    tagged_socket.send_enqueue(payload, saddr)
}

// TODO: this needs to be blocking
pub fn recv_from(socketfd: u16) -> Result<(Vec<u8>, SocketAddr)> {
    let sockets = &mut interface().sockets;

    // 1. check if a socketfd is valid if not return error
    let tagged_socket = sockets
        .get_mut(&socketfd)
        .ok_or(Error::InvalidSocket(socketfd))?;

    // 2. if socket not bound, return error
    if !tagged_socket.is_bound() {
        return Err(Error::InvalidSocket(socketfd));
    }

    // 3. blocking recv from socket recv queue
    tagged_socket.recv() // this needs to be blocking
}

pub fn bind(socketfd: u16, port: u16) -> Result<()> {
    // 1. check if binding is already in use by another socket
    let bind_addr = SocketAddr {
        addr: *interface().ipv4_addr,
        port,
    };
    for (_, socket) in &mut interface().sockets {
        if socket.binding_equals(bind_addr) {
            return Err(Error::BindingInUse(bind_addr));
        }
    }

    let sockets = &mut interface().sockets;
    // 2. check if this is a valid socketfd
    let tagged_socket = sockets
        .get_mut(&socketfd)
        .ok_or(Error::InvalidSocket(socketfd))?;

    // 3. bind the socket (will also overwrite current binding)
    tagged_socket.bind(port);

    Ok(())
}
