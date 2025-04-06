use crate::repr::{EthernetFrame, Ipv4Address, Ipv4Packet, UdpPacket};
use crate::iface::Interface;
use crate::socket::{Bindings, RawSocket, RawType, SocketAddr, UdpSocket};
use crate::utils::{ring::Ring, slice::Slice};
use crate::Result;

// use std::time::Instant;

// default max packet buffers
pub static RAW_SOCKET_PACKETS: usize = 128;
pub static UDP_SOCKET_PACKETS: usize = 128;

// environment for sockets on a specific interface
pub struct SocketEnv {
    bindings: Bindings,
    interface_mtu: usize,
}

impl SocketEnv {
    pub fn new(interface: &Interface) -> SocketEnv {
        SocketEnv {
            bindings: Bindings::new(),
            interface_mtu: interface.dev.mtu(),
        }
    }

    pub fn raw_socket(&self, raw_type: RawType) -> RawSocket {
        let header_len = match raw_type {
            RawType::Ethernet => EthernetFrame::<&[u8]>::HEADER_LEN,
            RawType::Ipv4 => {
                EthernetFrame::<&[u8]>::HEADER_LEN + Ipv4Packet::<&[u8]>::MIN_HEADER_LEN
            }
        };

        let payload_len = self.interface_mtu.checked_sub(header_len).unwrap();

        let buffer = || {
            let payload = Slice::from(vec![0; payload_len]);
            Ring::from(vec![payload; RAW_SOCKET_PACKETS])
        };

        RawSocket::new(raw_type, buffer(), buffer())
    }

    pub fn udp_socket(&self, socket_addr: SocketAddr) -> Result<UdpSocket> {
        let binding = self.bindings.bind_udp(socket_addr)?;

        let header_len = EthernetFrame::<&[u8]>::HEADER_LEN + Ipv4Packet::<&[u8]>::MIN_HEADER_LEN
            + UdpPacket::<&[u8]>::HEADER_LEN;

        let payload_len = self.interface_mtu.checked_sub(header_len).unwrap();

        let buffer = || {
            let payload = Slice::from(vec![0; payload_len]);
            let addr = SocketAddr {
                addr: Ipv4Address::new([0, 0, 0, 0]),
                port: 0,
            };
            Ring::from(vec![(payload, addr); UDP_SOCKET_PACKETS])
        };

        Ok(UdpSocket::new(binding, buffer(), buffer()))
    }

    // pub fn tcp_socket(&self, socket_addr: SocketAddr) -> Result<TcpSocket> {
    //     let binding = self.bindings.bind_tcp(socket_addr)?;
    //     Ok(TcpSocket::new(
    //         binding,
    //         self.interface_mtu,
    //     ))
    // }
}

