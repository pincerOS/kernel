#![no_std]
#![cfg_attr(not(test), no_main)]

pub extern crate display_proto as proto;

#[allow(unused_imports)]
#[macro_use]
extern crate ulib;

use proto::BufferHandle;
use ulib::sys::{mmap, recv, send};

pub fn connect(width: u16, height: u16) -> BufferHandle {
    let server_socket = 12;
    let message = ulib::sys::Message {
        tag: 0x101,
        objects: [u32::MAX, u32::MAX, u32::MAX, u32::MAX],
    };

    let mut buffer = [0; 8];
    buffer[0..2].copy_from_slice(&u16::to_ne_bytes(width));
    buffer[2..4].copy_from_slice(&u16::to_ne_bytes(height));

    send(server_socket, &message, &buffer, 0);

    let mut buf = [0u8; 64];
    let (_len, msg) = recv(server_socket, &mut buf, 0).unwrap();
    assert!(msg.tag == 0x100);
    let fd = msg.objects[0];

    let size = u64::from_le_bytes(buf[0..8].try_into().unwrap());
    let buffer = unsafe { mmap(0, size as usize, 0, 0, fd, 0) }.unwrap();
    let header = buffer.cast::<proto::BufferHeader>();

    let handle = unsafe { BufferHandle::new(header, &msg.objects) };
    handle
}

pub unsafe fn disconnect(_buf: *mut proto::BufferHeader) {
    // TODO: disconnect?
}
