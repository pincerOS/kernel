use crate::networking::iface::Interface;
use crate::networking::socket::{SocketAddr, TcpSocket, UdpSocket, SockType};
use crate::networking::{Error, Result};

use crate::device::usb::device::net::get_interface_mut;
use crate::ringbuffer::{Receiver, Sender};

use alloc::vec::Vec;

pub static BUFFER_LEN: usize = 128;

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

    pub async fn send_enqueue(&mut self, payload: Vec<u8>, saddr: SocketAddr) -> Result<()> {
        match self {
            // TaggedSocket::Raw(socket) => socket.queue_send(payload, saddr),
            TaggedSocket::Udp(socket) => socket.send_enqueue(payload, saddr).await,
            TaggedSocket::Tcp(socket) => socket.send_enqueue(payload, saddr),
        }
    }

    pub async fn recv_enqueue(
        &mut self,
        seq_num: u32,
        ack_num: u32,
        flags: u8,
        payload: Vec<u8>,
        saddr: SocketAddr,
    ) -> Result<()> {
        match self {
            // TaggedSocket::Raw(socket) => socket.queue_recv(payload, saddr),
            TaggedSocket::Udp(socket) => socket.recv_enqueue(payload, saddr).await,
            TaggedSocket::Tcp(socket) => {
                socket.recv_enqueue(seq_num, ack_num, flags, payload, saddr).await
            }
        }
    }

    pub fn get_recv_ref(&mut self) -> (SockType, Receiver<BUFFER_LEN, (Vec<u8>, SocketAddr)>) {
        match self {
            // TaggedSocket::Raw(socket) => socket.accepts(pair),
            TaggedSocket::Udp(socket) => socket.get_recv_ref(),
            TaggedSocket::Tcp(socket) => socket.get_recv_ref(),
        }
    }

    pub fn get_send_ref(&mut self) -> (SockType, Sender<BUFFER_LEN, (Vec<u8>, SocketAddr)>) {
        match self {
            // TaggedSocket::Raw(socket) => socket.accepts(pair),
            TaggedSocket::Udp(socket) => socket.get_send_ref(),
            TaggedSocket::Tcp(socket) => socket.get_send_ref(),
        }
    }

    pub async fn recv(&mut self) -> Result<(Vec<u8>, SocketAddr)> {
        match self {
            // TaggedSocket::Raw(socket) => socket.recv(),
            TaggedSocket::Udp(socket) => socket.recv().await,
            TaggedSocket::Tcp(socket) => socket.recv().await,
        }
    }

    pub fn binding_equals(&mut self, saddr: SocketAddr) -> bool {
        match self {
            // TaggedSocket::Raw(socket) => socket.recv(),
            TaggedSocket::Udp(socket) => socket.binding_equals(saddr),
            TaggedSocket::Tcp(socket) => socket.binding_equals(saddr),
        }
    }

    pub async fn connect(&mut self, saddr: SocketAddr) -> Result<()> {
        let interface = get_interface_mut();
        match self {
            // TaggedSocket::Raw(socket) => socket.recv(),
            TaggedSocket::Udp(_socket) => Err(Error::Ignored),
            TaggedSocket::Tcp(socket) => socket.connect(interface, saddr).await,
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
