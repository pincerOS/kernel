use crate::networking::utils::{ring::Ring, slice::Slice};
use crate::networking::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawType {
    Ethernet,
    Ipv4,
}

// Socket for sending and receiving raw ethernet or IP packets.
#[derive(Debug)]
pub struct RawSocket {
    raw_type: RawType,
    send_buffer: Ring<Slice<u8>>,
    recv_buffer: Ring<Slice<u8>>,
}

impl RawSocket {
    pub fn new(
        raw_type: RawType,
        send_buffer: Ring<Slice<u8>>,
        recv_buffer: Ring<Slice<u8>>,
    ) -> RawSocket {
        RawSocket {
            raw_type,
            send_buffer,
            recv_buffer,
        }
    }

    pub fn send(&mut self, buffer_len: usize) -> Result<&mut [u8]> {
        self.send_buffer.enqueue_maybe(|buffer| {
            buffer.try_resize(buffer_len, 0)?;

            for i in 0..buffer_len {
                buffer[i] = 0;
            }

            return Ok(&mut buffer[..buffer_len]);
        })
    }

    // Dequeues a received packet from the socket.
    pub fn recv(&mut self) -> Result<&[u8]> {
        self.recv_buffer.dequeue_with(|buffer| &buffer[..])
    }

    // Dequeues a packet enqueued for sending via a function f.
    //
    // The packet is only dequeued if f does not return an error.
    pub fn send_dequeue<F, R>(&mut self, f: F) -> Result<R>
    where
        F: FnOnce(&[u8]) -> Result<R>,
    {
        self.send_buffer.dequeue_maybe(|buffer| f(&buffer[..]))
    }

    // Enqueues a packet for receiving.
    pub fn recv_enqueue(&mut self, packet: &[u8]) -> Result<()> {
        self.recv_buffer.enqueue_maybe(|buffer| {
            buffer.try_resize(packet.len(), 0)?;
            buffer.copy_from_slice(packet);
            Ok(())
        })
    }

    pub fn raw_type(&self) -> RawType {
        self.raw_type
    }
}
