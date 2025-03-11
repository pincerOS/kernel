use core::storage::{Ring, Slice};
use Result;

// NOTE: this are my machine layers for now, but i dont really see the need for more maybe 802 in
// the future?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawType {
    Ethernet,
    Ipv4,
}

// this will contain the raw eth or ip packets
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

    // enqueue for sending
    pub fn send(&mut self, buffer_len: usize) -> Result<&mut [u8]> {
        self.send_buffer.enqueue_maybe(|buffer| {
            buffer.try_resize(buffer_len, 0)?;

            for i in 0 .. buffer_len {
                buffer[i] = 0;
            }

            return Ok(&mut buffer[.. buffer_len]);
        })
    }

    pub fn recv(&mut self) -> Result<&[u8]> {
        self.recv_buffer.dequeue_with(|buffer| &buffer[..])
    }

    // dequeue given a function f, ty alex for showing how to do this in async ipc lol
    pub fn send_dequeue<F, R>(&mut self, f: F) -> Result<R>
    where
        F: FnOnce(&[u8]) -> Result<R>,
    {
        self.send_buffer.dequeue_maybe(|buffer| f(&buffer[..]))
    }

    // enq a packet for receiving.
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
