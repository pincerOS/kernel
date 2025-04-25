use crate::device::usb::device::net::get_interface_mut;
use crate::networking::iface::udp;
use crate::networking::iface::Interface;
use crate::networking::socket::bindings::NEXT_SOCKETFD;
use crate::networking::socket::tagged::TaggedSocket;
use crate::networking::socket::SocketAddr;
use crate::networking::utils::ring::Ring;
use crate::networking::{Error, Result};

use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;

fn new_ring_packet_buffer(capacity: usize) -> Ring<(Vec<u8>, SocketAddr)> {
    let default_entry = (Vec::new(), SocketAddr::default()); // or some placeholder address
    let buffer = vec![default_entry; capacity];
    Ring::from(buffer)
}

pub static UDP_BUFFER_LEN: usize = 128;

// A UDP socket
pub struct UdpSocket {
    binding: SocketAddr,
    is_bound: bool,
    send_buffer: Ring<(Vec<u8>, SocketAddr)>,
    recv_buffer: Ring<(Vec<u8>, SocketAddr)>,
}

impl UdpSocket {
    pub fn new() -> u16 {
        let interface = get_interface_mut();
        let socket = UdpSocket {
            binding: SocketAddr {
                addr: *interface.ipv4_addr,
                port: 0,
            },
            is_bound: false,
            send_buffer: new_ring_packet_buffer(UDP_BUFFER_LEN),
            recv_buffer: new_ring_packet_buffer(UDP_BUFFER_LEN),
        };

        let socketfd = NEXT_SOCKETFD.fetch_add(1, Ordering::SeqCst);
        let mut sockets = interface.sockets.lock();
        sockets.insert(socketfd, TaggedSocket::Udp(socket));

        socketfd
    }

    pub fn binding_equals(&self, saddr: SocketAddr) -> bool {
        self.binding == saddr
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

    pub fn send_enqueue(&mut self, payload: Vec<u8>, dest: SocketAddr) -> Result<()> {
        self.send_buffer.enqueue_maybe(|(buffer, addr)| {
            *buffer = payload;
            *addr = dest;
            Ok(())
        })
    }

    pub fn send(&mut self, interface: &mut Interface) -> Result<()> {
        loop {
            match self.send_buffer.dequeue_with(|entry| {
                let (payload, addr) = entry;
                (payload.clone(), *addr)
            }) {
                Ok((payload, dest)) => {
                    let _ = udp::send_udp_packet(
                        interface,
                        dest.addr,
                        payload,
                        self.binding.port,
                        dest.port,
                    );
                }
                Err(Error::Exhausted) => break,
                Err(_) => break,
            }
        }

        Ok(())
    }

    // Dequeues a received packet along with it's source address from the
    // socket.
    // TODO: make blocking
    pub fn recv(&mut self) -> Result<(Vec<u8>, SocketAddr)> {
        self.recv_buffer
            .dequeue_with(|entry: &mut (Vec<u8>, SocketAddr)| {
                let (buffer, addr) = entry;
                (buffer.clone(), addr.clone())
            })
    }

    // Enqueues a packet for receiving.
    pub fn recv_enqueue(&mut self, payload: Vec<u8>, sender: SocketAddr) -> Result<()> {
        self.recv_buffer.enqueue_maybe(|(buffer, addr)| {
            *buffer = payload;
            *addr = sender;
            Ok(())
        })
    }

    // Returns the number of packets enqueued for sending.
    pub fn num_send_enqueued(&self) -> usize {
        self.send_buffer.len()
    }

    // Returns the number of packets enqueued for receiving.
    pub fn num_recv_enqueued(&self) -> usize {
        self.recv_buffer.len()
    }
}
