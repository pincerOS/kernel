use crate::socket::{ RawSocket, UdpSocket };

// One of many types of sockets.
pub enum TaggedSocket {
    Raw(RawSocket),
    Udp(UdpSocket),
    // Tcp(TcpSocket),
}

impl TaggedSocket {
    // panics
    pub fn as_raw_socket(&mut self) -> &mut RawSocket {
        match *self {
            TaggedSocket::Raw(ref mut socket) => socket,
            _ => panic!("Not a raw socket!"),
        }
    }

    // panics
    // pub fn as_tcp_socket(&mut self) -> &mut TcpSocket {
    //     match *self {
    //         TaggedSocket::Tcp(ref mut socket) => socket,
    //         _ => panic!("Not a TCP socket!"),
    //     }
    // }

    // panics
    pub fn as_udp_socket(&mut self) -> &mut UdpSocket {
        match *self {
            TaggedSocket::Udp(ref mut socket) => socket,
            _ => panic!("Not a UDP socket!"),
        }
    }
}
