use crate::repr::{Ipv4Protocol, Ipv4Repr, UdpPacket, UdpRepr};
use crate::socket::{SocketAddr, SocketAddrLease};
use crate::utils::{ring::Ring, slice::Slice};
use crate::{Error, Result};

// A UDP socket
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

    // is socket receiving to dest?
    pub fn accepts(&self, dst_addr: &SocketAddr) -> bool {
        &(*self.binding) == dst_addr
    }

    // Enqueues a packet with a payload_len bytes payload for sending to the
    // specified address.
    pub fn send(&mut self, buffer_len: usize, addr: SocketAddr) -> Result<&mut [u8]> {
        self.send_buffer
            .enqueue_maybe(|&mut (ref mut buffer, ref mut addr_)| {
                buffer.try_resize(buffer_len, 0)?;

                for i in 0 .. buffer_len {
                    buffer[i] = 0;
                }

                *addr_ = addr;

                return Ok(&mut buffer[.. buffer_len]);
            })
    }

    // Dequeues a received packet along with it's source address from the
    // socket.
    pub fn recv(&mut self) -> Result<(&[u8], SocketAddr)> {
        self.recv_buffer
            .dequeue_with(|&mut (ref buffer, ref addr)| (&buffer[..], addr.clone()))
    }

    // Dequeues a packet enqueued for sending via function f.
    //
    // The packet is only dequeued if f does not return an error.
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

    // Enqueues a packet for receiving.
    pub fn recv_enqueue(
        &mut self,
        ipv4_repr: &Ipv4Repr,
        udp_repr: &UdpRepr,
        payload: &[u8],
    ) -> Result<()> {
        let binding = self.binding.clone();
        self.recv_buffer
            .enqueue_maybe(|&mut (ref mut buffer, ref mut addr)| {
                if ipv4_repr.dst_addr != binding.addr || udp_repr.dst_port != binding.port {
                    Err(Error::Ignored)
                } else {
                    buffer.try_resize(payload.len(), 0)?;
                    buffer.copy_from_slice(payload);
                    addr.addr = ipv4_repr.src_addr;
                    addr.port = udp_repr.src_port;
                    Ok(())
                }
            })
    }

    // Returns the number of packets enqueued for sending.
    pub fn send_enqueued(&self) -> usize {
        self.send_buffer.len()
    }

    // Returns the number of packets enqueued for receiving.
    pub fn recv_enqueued(&self) -> usize {
        self.recv_buffer.len()
    }
}
