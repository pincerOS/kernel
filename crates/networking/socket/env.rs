use crate::repr::{EthernetFrame, Ipv4Address, Ipv4Packet, UdpPacket};
use crate::iface::Interface;
use crate::socket::{Bindings, RawSocket, RawType, SocketAddr, UdpSocket};

use core::storage::{Ring, Slice};
use core::time::Env as TimeEnv;

use Result;

// some constants for max buffer
pub static RAW_SOCKET_PACKETS: usize = 128;
pub static UDP_SOCKET_PACKETS: usize = 128;

// allows us to make an environment for each tagged socket
pub struct SocketEnv<T: 'static + TimeEnv + Clone> {
    bindings: Bindings,
    interface_mtu: usize,
    time_env: T,
}

impl<T: 'static + TimeEnv + Clone> SocketEnv<T> {
    pub fn new(interface: &Interface, time_env: T) -> SocketEnv<T> {
        SocketEnv {
            bindings: Bindings::new(),
            interface_mtu: interface.dev.max_transmission_unit(),
            time_env,
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

}
