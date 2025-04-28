use crate::device::usb::device::net::get_interface_mut;
use crate::networking::iface::udp;
use crate::networking::iface::Interface;
use crate::networking::socket::bindings::NEXT_SOCKETFD;
use crate::networking::socket::tagged::{TaggedSocket, BUFFER_LEN};
use crate::networking::socket::{SocketAddr, SockType};
use crate::ringbuffer::{channel, Sender, Receiver};
use crate::networking::{Error, Result};

use alloc::vec::Vec;
use core::sync::atomic::Ordering;

// A UDP socket
pub struct UdpSocket {
    binding: SocketAddr,
    is_bound: bool,
    // send_tx: Sender<UDP_BUFFER_LEN, (Vec<u8>, SocketAddr)>,
    // send_rx: Receiver<UDP_BUFFER_LEN, (Vec<u8>, SocketAddr)>,
    recv_tx: Sender<BUFFER_LEN, (Vec<u8>, SocketAddr)>,
    recv_rx: Receiver<BUFFER_LEN, (Vec<u8>, SocketAddr)>,
}

impl UdpSocket {
    pub fn new() -> u16 {
        let interface = get_interface_mut();
        
        // let (send_tx, send_rx) = channel::<UDP_BUFFER_LEN, (Vec<u8>, SocketAddr)>();
        let (recv_tx, recv_rx) = channel::<BUFFER_LEN, (Vec<u8>, SocketAddr)>();

        let socket = UdpSocket {
            binding: SocketAddr {
                addr: *interface.ipv4_addr,
                port: 0,
            },
            is_bound: false,
            // send_tx,
            // send_rx,
            recv_tx,
            recv_rx,
        };

        let socketfd = NEXT_SOCKETFD.fetch_add(1, Ordering::SeqCst);
        let mut sockets = interface.sockets.lock();
        sockets.insert(socketfd, TaggedSocket::Udp(socket));

        socketfd
    }

    pub fn binding_equals(&self, saddr: SocketAddr) -> bool {
        println!("binding port {} provided port {}", self.binding.port, saddr.port);
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

    pub async fn send_enqueue(&mut self, payload: Vec<u8>, dest: SocketAddr) -> Result<()> {
        println!("enqueud send");
        let interface = get_interface_mut();

        udp::send_udp_packet(interface, dest.addr, payload, self.binding.port, dest.port)
    }

    pub fn get_recv_ref(&mut self) -> (SockType, Receiver<BUFFER_LEN, (Vec<u8>, SocketAddr)>) {
        (SockType::UDP, self.recv_rx.clone())
    }

    pub fn get_send_ref(&mut self) -> (SockType, Sender<BUFFER_LEN, (Vec<u8>, SocketAddr)>) {
        (SockType::UDP, self.recv_tx.clone())
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

    pub fn num_send_enqueued(&self) -> usize {
        0
    }

    pub fn num_recv_enqueued(&self) -> usize {
        0
    }
}
