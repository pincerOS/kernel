use alloc::vec::Vec;
use core::{slice::IterMut as SliceIterMut};

use crate::networking::socket::TaggedSocket;
use crate::networking::SocketAddr;

// A set of sockets with stable integral handles.
pub struct SocketSet {
    sockets: Vec<Option<TaggedSocket>>,
    count: usize,
}

impl SocketSet {
    // Creates a socket set supporting a maximum number of sockets.
    pub fn new(socket_capacity: usize) -> SocketSet {
        SocketSet {
            sockets: (0..socket_capacity).map(|_| None).collect(),
            count: 0,
        }
    }

    pub fn port_open_udp(&mut self, dst_socket_addr: SocketAddr) -> Option<usize> {
        for (i, socket_opt) in self.sockets.iter_mut().enumerate() {
            if let Some(sock) = socket_opt {
                if sock.as_udp_socket().accepts(dst_socket_addr) {
                    return Some(i);
                }
            }
        }
        None
    }

    // pub fn port_open(&mut self, dst_socket_addr: SocketAddr, u8) -> Option<usize> {
    //     for (i, socket_opt) in self.sockets.iter_mut().enumerate() {
    //         if let Some(sock) = socket_opt {
    //             if sock.accepts(&dst_socket_addr) {
    //                 return Some(i);
    //             }
    //         }
    //     }
    //     None
    // }

    // add to set and return handle
    pub fn add_socket(&mut self, socket: TaggedSocket) -> Option<usize> {
        let handle = {
            (0..self.sockets.len())
                .filter(|i| self.sockets[*i].is_none())
                .next()
        };

        if let Some(i) = handle {
            self.sockets[i] = Some(socket);
            self.count += 1;
        }

        handle
    }

    // gives reference to socket in set otherwise panic
    pub fn socket(&mut self, socket_handle: usize) -> &mut TaggedSocket {
        if socket_handle >= self.sockets.len() {
            panic!("Socket handle is not in use.")
        } else {
            match self.sockets[socket_handle] {
                Some(ref mut socket) => socket,
                _ => panic!("Socket handle is not in use."),
            }
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn iter_mut(&mut self) -> SocketIter {
        SocketIter {
            inner: self.sockets.iter_mut(),
        }
    }
}

// An iterator over the sockets in a SocketSet.
pub struct SocketIter<'a> {
    inner: SliceIterMut<'a, Option<TaggedSocket>>,
}

impl<'a> Iterator for SocketIter<'a> {
    type Item = &'a mut TaggedSocket;

    fn next(&mut self) -> Option<&'a mut TaggedSocket> {
        while let Some(socket) = self.inner.next() {
            if let Some(ref mut socket) = *socket {
                return Some(socket);
            }
        }

        None
    }
}
