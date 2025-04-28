use core::fmt::{Display, Formatter, Result as FmtResult};
use core::hash::Hash;
use core::sync::atomic::{AtomicU16, Ordering};

use alloc::vec::Vec;

use crate::networking::repr::Ipv4Address;
use crate::networking::{Error, Result};

use crate::device::usb::device::net::get_interface_mut;
use crate::event::task::spawn_async;

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

#[derive(Debug, Eq, PartialEq)]
pub enum SockType {
    UDP,
    TCP,
    Raw
}

// TODO: these technically runs out eventually lol need wrap around
pub static NEXT_EPHEMERAL: AtomicU16 = AtomicU16::new(32768);
pub static NEXT_SOCKETFD: AtomicU16 = AtomicU16::new(1);

pub async fn send_to(socketfd: u16, payload: Vec<u8>, saddr: SocketAddr) -> Result<()> {
    let interface = get_interface_mut();
    // let mut sockets = interface.sockets.lock();

    // 1. check if socket fd is valid if not return error
    let tagged_socket = interface.sockets
        .get_mut(&socketfd)
        .ok_or(Error::InvalidSocket(socketfd))?;

    // 2. if socket not bound, bind to ephemeral port (32768–60999)
    if !tagged_socket.is_bound() {
        let ephem_port = NEXT_EPHEMERAL.fetch_add(1, Ordering::SeqCst);

        tagged_socket.bind(ephem_port);
    }

    // 3. queue a send on socket sending queue
    tagged_socket.send_enqueue(payload, saddr).await
}

pub async fn recv_from(socketfd: u16) -> Result<(Vec<u8>, SocketAddr)> {
    let interface = get_interface_mut();

    // 1. check if a socketfd is valid if not return error
    let tagged_socket = interface.sockets
        .get_mut(&socketfd)
        .ok_or(Error::InvalidSocket(socketfd))?;

    // 2. if socket not bound, return error
    if !tagged_socket.is_bound() {
        return Err(Error::InvalidSocket(socketfd));
    }

    // 3. blocking recv from socket recv queue
    
    tagged_socket.recv().await
}

pub async fn connect(socketfd: u16, saddr: SocketAddr) -> Result<()> {
    let interface = get_interface_mut();
    // let mut sockets = interface.sockets.lock();

    // 1. check if a socketfd is valid if not return error
    let tagged_socket = interface.sockets
        .get_mut(&socketfd)
        .ok_or(Error::InvalidSocket(socketfd))?;

    tagged_socket.connect(saddr).await
}

pub async fn listen(socketfd: u16, num_requests: usize) -> Result<()> {
    let interface = get_interface_mut();
    // 1.check if binded, if not error
    // let mut sockets = interface.sockets.lock();

    let tagged_socket = interface.sockets
        .get_mut(&socketfd)
        .ok_or(Error::InvalidSocket(socketfd))?;

    if !tagged_socket.is_bound() {
        return Err(Error::InvalidSocket(socketfd));
    }

    // 2. start the listener
    tagged_socket.listen(num_requests).await
}

pub async fn accept(socketfd: u16) -> Result<u16> {
    let interface = get_interface_mut();
    // 1. if listener not started, error
    // let mut sockets = interface.sockets.lock();

    let tagged_socket = interface.sockets
        .get_mut(&socketfd)
        .ok_or(Error::InvalidSocket(socketfd))?;

    // 2. accept 1 connection, error if no pending connections
    tagged_socket.accept().await;
    Ok(socketfd)
}

pub async fn close(socketfd: u16) -> Result<()> {
    let interface = get_interface_mut();
    // 1. if listener not started, error
    // let mut sockets = interface.sockets.lock();

    let tagged_socket = interface.sockets
        .get_mut(&socketfd)
        .ok_or(Error::InvalidSocket(socketfd))?;

    // 2. accept 1 connection, error if no pending connections
    tagged_socket.close().await
}

pub fn bind(socketfd: u16, port: u16) -> Result<()> {
    let interface = get_interface_mut();
    // 1. check if binding is already in use by another socket
    let bind_addr = SocketAddr {
        addr: *interface.ipv4_addr,
        port,
    };
    // let mut sockets = interface.sockets.lock();
    for (_, socket) in interface.sockets.iter_mut() {
        if socket.binding_equals(bind_addr) {
            return Err(Error::BindingInUse(bind_addr));
        }
    }

    // 2. check if this is a valid socketfd
    let tagged_socket = interface.sockets
        .get_mut(&socketfd)
        .ok_or(Error::InvalidSocket(socketfd))?;

    // 3. bind the socket (will also overwrite current binding)
    tagged_socket.bind(port);

    Ok(())
}
