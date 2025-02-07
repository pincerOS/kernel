use alloc::boxed::Box;

use crate::ringbuffer;

pub struct ObjectDescriptor {
    pub value: core::num::NonZeroU32,
}

impl ObjectDescriptor {
    pub fn new(value: core::num::NonZeroU32) -> Self {
        ObjectDescriptor{ value }
    }
}

pub struct Message {
    tag: u64,
    objects: [Option<ObjectDescriptor>; 4],
    data: Option<Box<[u8]>>,
}

pub struct Channel {
    send: ringbuffer::Sender<16, Message>,
    recv: ringbuffer::Receiver<16, Message>,
}

impl Channel {
    pub fn new(send: ringbuffer::Sender<16, Message>, recv: ringbuffer::Receiver<16, Message>) -> Self {
        Channel{send, recv}
    }
}

pub enum Object {
    Channel(Channel),
}
