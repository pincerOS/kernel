use crate::networking::socket::{IpAddrPair, UdpSocket};

use alloc::sync::Arc;

pub enum TaggedSocket {
    // Raw(RawSocket),
    Udp(Arc<UdpSocket>),
    // Tcp(Arc<TcpSocket>),
}

impl TaggedSocket {
    pub fn accepts(&mut self, pair: IpAddrPair) -> bool {
        match self {
            // TaggedSocket::Raw(socket) => socket.accepts(pair),
            TaggedSocket::Udp(socket) => socket.accepts(pair),
            // TaggedSocket::Tcp(socket) => socket.accepts(pair),
        }
    }
}
