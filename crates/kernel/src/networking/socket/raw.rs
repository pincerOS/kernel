use crate::device::usb::device::net::get_interface_mut;
use crate::networking::iface::ipv4;
use crate::networking::iface::Interface;
use crate::networking::repr::Ipv4Protocol;
use crate::networking::socket::tagged::BUFFER_LEN;
use crate::networking::socket::{SockType, SocketAddr};
use crate::networking::{Error, Result};
use crate::ringbuffer::{channel, Receiver, Sender};

use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawType {
    Ethernet,
    Ipv4,
}

// Socket for sending and receiving raw ethernet or IP packets.
pub struct RawSocket {
    raw_type: RawType,
    recv_tx: Sender<BUFFER_LEN, (Vec<u8>, SocketAddr)>,
    recv_rx: Receiver<BUFFER_LEN, (Vec<u8>, SocketAddr)>,
    is_bound: bool,
    binding: SocketAddr,
}

impl RawSocket {
    pub fn new(raw_type: RawType) -> RawSocket {
        let (recv_tx, recv_rx) = channel::<BUFFER_LEN, (Vec<u8>, SocketAddr)>();
        let interface = get_interface_mut();
        RawSocket {
            raw_type,
            recv_tx,
            recv_rx,
            is_bound: false,
            binding: SocketAddr {
                addr: *interface.ipv4_addr,
                port: 0,
            },
        }
    }

    pub fn binding_equals(&self, saddr: SocketAddr) -> bool {
        self.binding.port == saddr.port
    }

    pub fn is_bound(&self) -> bool {
        self.is_bound
    }

    pub fn bind(&mut self, interface: &mut Interface, port: u16) {
        self.is_bound = true;
        let bind_addr = SocketAddr {
            addr: *interface.ipv4_addr,
            port,
        };
        self.binding = bind_addr;
    }

    pub async fn send_enqueue(
        &mut self,
        payload: Vec<u8>,
        proto: Ipv4Protocol,
        dest: SocketAddr,
    ) -> Result<()> {
        println!("enqueud send");
        let interface = get_interface_mut();

        ipv4::send_ipv4_packet(interface, payload, proto, dest.addr)
    }

    pub fn get_recv_ref(&mut self) -> (SockType, Receiver<BUFFER_LEN, (Vec<u8>, SocketAddr)>) {
        (SockType::Raw, self.recv_rx.clone())
    }

    pub fn get_send_ref(&mut self) -> (SockType, Sender<BUFFER_LEN, (Vec<u8>, SocketAddr)>) {
        (SockType::Raw, self.recv_tx.clone())
    }

    pub async fn recv(&mut self) -> Result<(Vec<u8>, SocketAddr)> {
        let (payload, addr) = self.recv_rx.recv().await;
        Ok((payload, addr))
    }

    pub fn try_recv(&mut self) -> Result<(Vec<u8>, SocketAddr)> {
        match self.recv_rx.try_recv() {
            Some((payload, addr)) => Ok((payload, addr)),
            None => Err(Error::Exhausted),
        }
    }

    pub async fn recv_enqueue(&mut self, payload: Vec<u8>, sender: SocketAddr) -> Result<()> {
        println!("got a recv_enqueue");
        self.recv_tx.send((payload, sender)).await;
        Ok(())
    }

    pub fn raw_type(&self) -> RawType {
        self.raw_type
    }
}
