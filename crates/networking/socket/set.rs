use crate::socket::TaggedSocket;

pub struct SocketSet {
    sockets: Vec<Option<TaggedSocket>>,
    count: usize,
}

impl SocketSet {
    pub fn new(socket_capacity: usize) -> SocketSet {
        SocketSet {
            sockets: (0 .. socket_capacity).map(|_| None).collect(),
            count: 0,
        }
    }

    pub fn add_socket(&mut self, socket: TaggedSocket) -> Option<usize> {
        let handle = {
            (0 .. self.sockets.len())
                .filter(|i| self.sockets[*i].is_none())
                .next()
        };

        if let Some(i) = handle {
            self.sockets[i] = Some(socket);
            self.count += 1;
        }

        handle
    }

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

}

// NOTE: i saw some examples that make an iterator for this, probably should do that
