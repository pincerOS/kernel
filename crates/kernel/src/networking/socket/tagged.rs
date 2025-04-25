use crate::networking::socket::{SocketAddr, TcpSocket, UdpSocket};
use crate::networking::{Error, Result};
use crate::networking::iface::Interface;

use crate::device::usb::device::net::get_interface_mut;

use alloc::vec::Vec;

pub enum TaggedSocket {
    // Raw(RawSocket),
    Udp(UdpSocket),
    Tcp(TcpSocket),
}

impl TaggedSocket {
    pub fn is_bound(&mut self) -> bool {
        match self {
            // TaggedSocket::Raw(socket) => socket.accepts(pair),
            TaggedSocket::Udp(socket) => socket.is_bound(),
            TaggedSocket::Tcp(socket) => socket.is_bound(),
        }
    }

    pub fn bind(&mut self, port: u16) {
        let interface = get_interface_mut();
        match self {
            // TaggedSocket::Raw(socket) => socket.accepts(pair),
            TaggedSocket::Udp(socket) => socket.bind(interface, port),
            TaggedSocket::Tcp(socket) => socket.bind(interface, port),
        }
    }

    pub fn send(&mut self, interface: &mut Interface) -> Result<()> {
        match self {
            // TaggedSocket::Raw(socket) => socket.send(),
            TaggedSocket::Udp(socket) => socket.send(interface),
            TaggedSocket::Tcp(socket) => socket.send(interface),
        }
    }

    pub fn send_enqueue(&mut self, payload: Vec<u8>, saddr: SocketAddr) -> Result<()> {
        match self {
            // TaggedSocket::Raw(socket) => socket.queue_send(payload, saddr),
            TaggedSocket::Udp(socket) => socket.send_enqueue(payload, saddr),
            TaggedSocket::Tcp(socket) => socket.send_enqueue(payload, saddr),
        }
    }

    // TODO: this is so ugl lol
    pub fn recv_enqueue(
        &mut self,
        seq_num: u32,
        ack_num: u32,
        flags: u8,
        payload: Vec<u8>,
        saddr: SocketAddr,
    ) -> Result<()> {
        let interface = get_interface_mut();

        match self {
            // TaggedSocket::Raw(socket) => socket.queue_recv(payload, saddr),
            TaggedSocket::Udp(socket) => socket.recv_enqueue(payload, saddr),
            TaggedSocket::Tcp(socket) => {
                socket.recv_enqueue(interface, seq_num, ack_num, flags, payload, saddr)
            }
        }
    }

    pub fn recv(&mut self) -> Result<(Vec<u8>, SocketAddr)> {
        match self {
            // TaggedSocket::Raw(socket) => socket.recv(),
            TaggedSocket::Udp(socket) => socket.recv(),
            TaggedSocket::Tcp(socket) => socket.recv(),
        }
    }

    pub fn binding_equals(&mut self, saddr: SocketAddr) -> bool {
        match self {
            // TaggedSocket::Raw(socket) => socket.recv(),
            TaggedSocket::Udp(socket) => socket.binding_equals(saddr),
            TaggedSocket::Tcp(socket) => socket.binding_equals(saddr),
        }
    }

    // TODO: should block
    // TODO: udp just throws error for now, but can be used like berkley posix to instead set the
    // default destination as well in the future
    pub fn connect(&mut self, saddr: SocketAddr) -> Result<()> {
        let interface = get_interface_mut();
        match self {
            // TaggedSocket::Raw(socket) => socket.recv(),
            TaggedSocket::Udp(_socket) => Err(Error::Ignored),
            TaggedSocket::Tcp(socket) => socket.connect(interface, saddr),
        }
    }

    pub fn listen(&mut self, num_req: usize) -> Result<()> {
        let interface = get_interface_mut();
        match self {
            // TaggedSocket::Raw(socket) => socket.recv(),
            TaggedSocket::Udp(_socket) => Err(Error::Ignored),
            TaggedSocket::Tcp(socket) => socket.listen(interface, num_req),
        }
    }

    pub fn accept(&mut self) -> Result<SocketAddr> {
        match self {
            // TaggedSocket::Raw(socket) => socket.recv(),
            TaggedSocket::Udp(_socket) => Err(Error::Ignored),
            TaggedSocket::Tcp(socket) => socket.accept(),
        }
    }
}
