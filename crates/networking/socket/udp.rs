// NOTE: THIS DOES NOT WORK, IT IS A WORK IN PROGRESS
use crate::repr::{Ipv4Protocol, Ipv4Repr, UdpPacket, UdpRepr};
use crate::socket::{ SocketAddr, SocketAddrLease};
use core::storage::{Ring, Slice};
use {Error, Result};

pub struct UdpSocket {
    binding: SocketAddrLease,
    send_buffer: Ring<(Slice<u8>, SocketAddr)>,
    recv_buffer: Ring<(Slice<u8>, SocketAddr)>,
}

impl UdpSocket {
    pub fn new(
        binding: SocketAddrLease,
        send_buffer: Ring<(Slice<u8>, SocketAddr)>,
        recv_buffer: Ring<(Slice<u8>, SocketAddr)>,
    ) -> UdpSocket {
        UdpSocket {
            binding,
            send_buffer,
            recv_buffer,
        }
    }

    pub fn accepts(&self, dst_addr: &SocketAddr) -> bool {
        &(*self.binding) == dst_addr
    }

    pub fn send(&mut self, buffer_len: usize, addr: SocketAddr) -> Result<&mut [u8]> {
        // TODO:: fill
    }

    pub fn recv(&mut self) -> Result<(&[u8], SocketAddr)> {
        self.recv_buffer
            .dequeue_with(|&mut (ref buffer, ref addr)| (&buffer[..], addr.clone()))
    }

    pub fn send_dequeue<F, R>(&mut self, f: F) -> Result<R>
    where
        F: FnOnce(&Ipv4Repr, &UdpRepr, &[u8]) -> Result<R>,
    {
        let binding = self.binding.clone();
        self.send_buffer
            .dequeue_maybe(|&mut (ref mut buffer, addr)| {
                let payload_len = buffer.len();

                let udp_repr = UdpRepr {
                    src_port: binding.port,
                    dst_port: addr.port,
                    length: UdpPacket::<&[u8]>::buffer_len(payload_len) as u16,
                };

                let ipv4_repr = Ipv4Repr {
                    src_addr: binding.addr,
                    dst_addr: addr.addr,
                    protocol: Ipv4Protocol::UDP,
                    payload_len: udp_repr.buffer_len() as u16,
                };

                f(&ipv4_repr, &udp_repr, &buffer[..])
            })
    }

    pub fn recv_enqueue(
        &mut self,
        ipv4_repr: &Ipv4Repr,
        udp_repr: &UdpRepr,
        payload: &[u8],
    ) -> Result<()> {
        // TODO:: fill
    }

    pub fn send_enqueued(&self) -> usize {
        self.send_buffer.len()
    }

    pub fn recv_enqueued(&self) -> usize {
        self.recv_buffer.len()
    }
}
