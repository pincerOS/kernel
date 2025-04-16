use crate::networking::iface::udp;
use crate::networking::repr::{Ipv4Packet, Ipv4Protocol, UdpPacket};
use crate::networking::socket::SocketAddr;
use crate::networking::utils::{ring::Ring, slice::Slice};
use crate::networking::{Error, Result};

use crate::event::thread;
use crate::device::usb::device::net::interface;

use alloc::vec::Vec;
use alloc::vec;

fn new_ring_packet_buffer(capacity: usize) -> Ring<(Vec<u8>, SocketAddr)> {
    let default_entry = (Vec::new(), SocketAddr::default()); // or some placeholder address
    let buffer = vec![default_entry; capacity];
    Ring::from(buffer)
}

// A UDP socket
pub struct UdpSocket {
    binding: SocketAddr,
    send_buffer: Ring<(Vec<u8>, SocketAddr)>,
    recv_buffer: Ring<(Vec<u8>, SocketAddr)>,
}

impl UdpSocket {
    pub fn new(
        binding: SocketAddr,
        capacity: usize,
    ) -> UdpSocket {
        UdpSocket {
            binding,
            send_buffer: new_ring_packet_buffer(capacity),
            recv_buffer: new_ring_packet_buffer(capacity),
        }
    }

    pub fn accepts(&self, dst_addr: SocketAddr) -> bool {
        self.binding == dst_addr
    }

    pub fn send(&mut self, payload: Vec<u8>, dest: SocketAddr) -> Result<()> {
        let src_port = self.binding.port;
        let payload_clone = payload.clone(); 

        self.recv_buffer.enqueue_maybe(|(buffer, addr)| {
            *buffer = payload; 
            *addr = dest;
            Ok(())
        });
        
        thread::thread(move || {
            udp::send_udp_packet(interface(), dest.addr, payload_clone, src_port, dest.port);
        });


        Ok(())
    }

    // Dequeues a received packet along with it's source address from the
    // socket.
    pub fn recv(&mut self) -> Result<(&[u8], SocketAddr)> {
        self.recv_buffer
            .dequeue_with(|&mut (ref buffer, ref addr)| (&buffer[..], addr.clone()))
    }

    // Enqueues a packet for receiving.
    pub fn recv_enqueue (
        &mut self,
        payload: Vec<u8>,
        sender: SocketAddr,
    ) -> Result<()> {

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
