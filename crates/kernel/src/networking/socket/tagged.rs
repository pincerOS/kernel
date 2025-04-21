use crate::networking::socket::{SocketAddr, UdpSocket};
use crate::networking::{Error, Result};

use alloc::vec::Vec;

pub enum TaggedSocket {
    // Raw(RawSocket),
    Udp(UdpSocket),
    // Tcp(Arc<TcpSocket>),
}

impl TaggedSocket {
    pub fn is_bound(&mut self) -> bool {
        match self {
            // TaggedSocket::Raw(socket) => socket.accepts(pair),
            TaggedSocket::Udp(socket) => socket.is_bound(),
            // TaggedSocket::Tcp(socket) => socket.accepts(pair),
        }
    }

    pub fn bind(&mut self, port: u16) {
        match self {
            // TaggedSocket::Raw(socket) => socket.accepts(pair),
            TaggedSocket::Udp(socket) => socket.bind(port),
            // TaggedSocket::Tcp(socket) => socket.accepts(pair),
        }
    }

    pub fn send(&mut self) {
        match self {
            // TaggedSocket::Raw(socket) => socket.send(),
            TaggedSocket::Udp(socket) => socket.send(),
            // TaggedSocket::Tcp(socket) => socket.send(),
        }
    }

    pub fn send_enqueue(&mut self, payload: Vec<u8>, saddr: SocketAddr) -> Result<()> {
        match self {
            // TaggedSocket::Raw(socket) => socket.queue_send(payload, saddr),
            TaggedSocket::Udp(socket) => socket.send_enqueue(payload, saddr),
            // TaggedSocket::Tcp(socket) => socket.queue_send(payload, saddr),
        }
    }

    pub fn recv_enqueue(&mut self, payload: Vec<u8>, saddr: SocketAddr) -> Result<()> {
        match self {
            // TaggedSocket::Raw(socket) => socket.queue_recv(payload, saddr),
            TaggedSocket::Udp(socket) => socket.recv_enqueue(payload, saddr),
            // TaggedSocket::Tcp(socket) => socket.queue_recv(payload, saddr),
        }
    }

    pub fn recv(&mut self) -> Result<(Vec<u8>, SocketAddr)> {
        match self {
            // TaggedSocket::Raw(socket) => socket.recv(),
            TaggedSocket::Udp(socket) => socket.recv(),
            // TaggedSocket::Tcp(socket) => socket.recv(),
        }
    }

    pub fn binding_equals(&mut self, saddr: SocketAddr) -> bool {
        match self {
            // TaggedSocket::Raw(socket) => socket.recv(),
            TaggedSocket::Udp(socket) => socket.binding_equals(saddr),
            // TaggedSocket::Tcp(socket) => socket.recv(),
        }
    }
}
