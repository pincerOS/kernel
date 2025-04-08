use crate::networking::socket::{ RawSocket, UdpSocket };

// One of many types of sockets.
pub enum TaggedSocket {
    Raw(RawSocket),
    Udp(UdpSocket),
    // Tcp(TcpSocket),
}

impl TaggedSocket {
    pub fn as_raw_socket(&mut self) -> &mut RawSocket {
        match *self {
            TaggedSocket::Raw(ref mut socket) => socket,
            _ => panic!("Not a raw socket!"),
        }
    }

    pub fn as_udp_socket(&mut self) -> &mut UdpSocket {
        match *self {
            TaggedSocket::Udp(ref mut socket) => socket,
            _ => panic!("Not a UDP socket!"),
        }
    }
}
