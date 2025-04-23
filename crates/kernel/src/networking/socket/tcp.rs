use crate::device::usb::device::net::interface;
use crate::networking::iface::tcp;
use crate::networking::socket::bindings::{NEXT_EPHEMERAL, NEXT_SOCKETFD};
use crate::networking::socket::tagged::TaggedSocket;
use crate::networking::socket::SocketAddr;
use crate::networking::utils::ring::Ring;
use crate::networking::{Error, Result};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;

fn new_ring_packet_buffer(capacity: usize) -> Ring<(Vec<u8>, SocketAddr)> {
    let default_entry = (Vec::new(), SocketAddr::default());
    let buffer = vec![default_entry; capacity];
    Ring::from(buffer)
}

pub static TCP_BUFFER_LEN: usize = 128;

// flags
pub const TCP_FLAG_FIN: u8 = 0x01;
pub const TCP_FLAG_SYN: u8 = 0x02;
pub const TCP_FLAG_RST: u8 = 0x04;
pub const TCP_FLAG_PSH: u8 = 0x08;
pub const TCP_FLAG_ACK: u8 = 0x10;

const INITIAL_SEQ_NUMBER: u32 = 1000; // TODO: maybe do a random initialization
const DEFAULT_WINDOW_SIZE: u16 = 8192;

#[derive(PartialEq, Eq)]
pub enum TcpState {
    Closed,      // initial state, no connection
    SynSent,     // connect initiated, waiting for SYN-ACK
    SynReceived, // SYN received, waiting for ACK
    Established, // connection established
    FinWait1,    // close initiated, waiting for FIN or FIN-ACK
    FinWait2,    // our FIN acknowledged, waiting for FIN
    CloseWait,   // received FIN, waiting for application close
    LastAck,     // sent our FIN, waiting for final ACK
    Closing,     // both sides initiated close simultaneously
    TimeWait,    // wait for duplicate packets to expire
}

pub struct TcpSocket {
    binding: SocketAddr,
    is_bound: bool,
    is_listener: bool,
    pending_conn: Vec<SocketAddr>,
    max_pending: usize,
    connected: bool,
    send_buffer: Ring<(Vec<u8>, SocketAddr)>,
    recv_buffer: Ring<(Vec<u8>, SocketAddr)>,

    state: TcpState,
    remote_addr: Option<SocketAddr>,
    seq_number: u32,
    ack_number: u32,
    window_size: u16,
}

impl TcpSocket {
    pub fn new() -> u16 {
        let socket = TcpSocket {
            binding: SocketAddr {
                addr: *interface().ipv4_addr,
                port: 0,
            },
            is_bound: false,
            is_listener: false,
            pending_conn: Vec::new(),
            max_pending: 0,
            connected: false,
            send_buffer: new_ring_packet_buffer(TCP_BUFFER_LEN),
            recv_buffer: new_ring_packet_buffer(TCP_BUFFER_LEN),

            state: TcpState::Closed,
            remote_addr: None,
            seq_number: INITIAL_SEQ_NUMBER,
            ack_number: 0,
            window_size: DEFAULT_WINDOW_SIZE,
        };

        let socketfd = NEXT_SOCKETFD.fetch_add(1, Ordering::SeqCst);
        interface()
            .sockets
            .insert(socketfd, TaggedSocket::Tcp(socket));
        socketfd
    }

    pub fn binding_equals(&self, saddr: SocketAddr) -> bool {
        self.binding == saddr
    }

    pub fn is_bound(&self) -> bool {
        self.is_bound
    }

    pub fn bind(&mut self, port: u16) {
        self.is_bound = true;
        let bind_addr = SocketAddr {
            addr: *interface().ipv4_addr,
            port,
        };
        self.binding = bind_addr;
    }

    pub fn listen(&mut self, num_max_requests: usize) -> Result<()> {
        if !self.is_bound {
            // bind to ephemeral if not bound
            let ephemeral_port = NEXT_EPHEMERAL.fetch_add(1, Ordering::SeqCst);
            self.bind(ephemeral_port as u16);
        }

        self.is_listener = true;
        self.max_pending = num_max_requests;
        self.state = TcpState::Closed; // still in CLOSED until SYN received
        self.pending_conn = Vec::with_capacity(num_max_requests);
        Ok(())
    }

    pub fn accept(&mut self) -> Result<SocketAddr> {
        if !self.is_listener {
            return Err(Error::NotConnected);
        }

        match self.pending_conn.pop() {
            Some(addr) => {
                self.remote_addr = Some(addr);
                Ok(addr)
            }
            None => Err(Error::Exhausted),
        }
    }

    pub fn connect(&mut self, saddr: SocketAddr) -> Result<()> {
        // make sure we're not already connected
        // match self.state {
        //     TcpState::Closed => {},
        //     _ => return Err(Error::AlreadyConnected),
        // }

        // if not already bound, bind to an ephemeral port
        if !self.is_bound {
            let ephemeral_port = NEXT_EPHEMERAL.fetch_add(1, Ordering::SeqCst);
            self.bind(ephemeral_port as u16);
        }
        self.is_bound = true;

        self.remote_addr = Some(saddr);

        let flags = TCP_FLAG_SYN;
        tcp::send_tcp_packet(
            interface(),
            self.binding.port,
            saddr.port,
            self.seq_number,
            0, // no ACK yet
            flags,
            self.window_size,
            saddr.addr,
            Vec::new(), // no payload
        )?;

        self.state = TcpState::SynSent;

        println!("[!] sent syn");

        Ok(())
    }

    pub fn send_enqueue(&mut self, payload: Vec<u8>, dest: SocketAddr) -> Result<()> {
        if self.state != TcpState::Established {
            return Err(Error::NotConnected);
        }

        // verify the destination matches the connected remote address
        if let Some(remote) = self.remote_addr {
            if remote != dest {
                return Err(Error::NotConnected);
            }
        } else {
            return Err(Error::NotConnected);
        }

        self.send_buffer.enqueue_maybe(|(buffer, addr)| {
            *buffer = payload;
            *addr = dest;
            Ok(())
        })
    }

    pub fn send(&mut self) -> Result<()> {
        if self.state != TcpState::Established {
            return Err(Error::NotConnected);
        }

        match self.state {
            TcpState::Established => {
                // Process outgoing data
                loop {
                    match self.send_buffer.dequeue_with(|entry| {
                        let (payload, addr) = entry;
                        (payload.clone(), *addr)
                    }) {
                        Ok((payload, dest)) => {
                            // Send with appropriate TCP flags
                            tcp::send_tcp_packet(
                                interface(),
                                self.binding.port,
                                dest.port,
                                self.seq_number,
                                self.ack_number,
                                TCP_FLAG_ACK | TCP_FLAG_PSH, // PSH to push data to application layer
                                self.window_size,
                                dest.addr,
                                payload.clone(),
                            )?;

                            // Update sequence number
                            self.seq_number += payload.len() as u32;
                        }
                        Err(Error::Exhausted) => break,
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            }
            _ => Err(Error::NotConnected),
        }
    }

    pub fn recv(&mut self) -> Result<(Vec<u8>, SocketAddr)> {
        self.recv_buffer
            .dequeue_with(|entry: &mut (Vec<u8>, SocketAddr)| {
                let (buffer, addr) = entry;
                (buffer.clone(), addr.clone())
            })
    }

    // Enqueues a packet for receiving and handles TCP state machine
    pub fn recv_enqueue(
        &mut self,
        seq_number: u32,
        ack_number: u32,
        flags: u8,
        payload: Vec<u8>,
        sender: SocketAddr,
    ) -> Result<()> {
        // Handle connection establishment if in SYN_SENT state
        if self.state == TcpState::SynSent {
            // Check if this is a valid remote endpoint response
            if let Some(remote) = self.remote_addr {
                if remote == sender {
                    // This is a response to our SYN
                    if (flags & (TCP_FLAG_SYN | TCP_FLAG_ACK)) == (TCP_FLAG_SYN | TCP_FLAG_ACK) {
                        // Received SYN-ACK, update ACK number
                        self.ack_number = seq_number + 1;

                        // Send ACK to complete three-way handshake
                        tcp::send_tcp_packet(
                            interface(),
                            self.binding.port,
                            sender.port,
                            self.seq_number + 1, // SYN consumes one sequence number
                            self.ack_number,
                            TCP_FLAG_ACK,
                            self.window_size,
                            sender.addr,
                            Vec::new(),
                        )?;

                        // Update state and sequence number
                        self.state = TcpState::Established;
                        self.connected = true;
                        self.seq_number += 1; // SYN consumes one sequence number

                        return Ok(());
                    }
                }
            }
        } else if self.state == TcpState::Closed {
            // Handle incoming SYN for passive open (if we're listening)
            if flags & TCP_FLAG_SYN != 0 && flags & TCP_FLAG_ACK == 0 {
                // This would be for a server socket - not handling passive open in this example
                // But this is where you would handle it
            }
        }

        // Process state transitions based on TCP flags
        self.process_tcp_state_transitions(flags, seq_number, ack_number, sender)?;

        // Now that we've handled any state transitions, enqueue actual data for user
        // Only enqueue if we're in established state and there's actual data
        if self.state == TcpState::Established && (payload.len() > 0) {
            // Only enqueue if there's actual data
            self.recv_buffer.enqueue_maybe(|(buffer, addr)| {
                *buffer = payload.clone();
                *addr = sender;
                Ok(())
            })?;

            // Update ACK number and send ACK for the data
            self.ack_number = seq_number + payload.len() as u32;

            // Send ACK for received data
            if let Some(remote) = self.remote_addr {
                tcp::send_tcp_packet(
                    interface(),
                    self.binding.port,
                    remote.port,
                    self.seq_number,
                    self.ack_number,
                    TCP_FLAG_ACK,
                    self.window_size,
                    remote.addr,
                    Vec::new(),
                )?;
            }
        }

        Ok(())
    }

    // Helper function to process TCP state transitions based on packet flags
    fn process_tcp_state_transitions(
        &mut self,
        flags: u8,
        seq_number: u32,
        _ack_number: u32,
        _sender: SocketAddr,
    ) -> Result<()> {
        match self.state {
            TcpState::Established => {
                // Handle FIN from remote
                if flags & TCP_FLAG_FIN != 0 {
                    self.ack_number = seq_number + 1; // FIN consumes a sequence number

                    // Send ACK for FIN
                    if let Some(remote) = self.remote_addr {
                        tcp::send_tcp_packet(
                            interface(),
                            self.binding.port,
                            remote.port,
                            self.seq_number,
                            self.ack_number,
                            TCP_FLAG_ACK,
                            self.window_size,
                            remote.addr,
                            Vec::new(),
                        )?;
                    }

                    self.state = TcpState::CloseWait;
                }

                // Handle RST from remote
                if flags & TCP_FLAG_RST != 0 {
                    self.state = TcpState::Closed;
                    self.connected = false;
                    self.remote_addr = None;
                }
            }

            TcpState::FinWait1 => {
                if flags & TCP_FLAG_ACK != 0 {
                    // Our FIN was acknowledged
                    if flags & TCP_FLAG_FIN != 0 {
                        // Simultaneous FIN-ACK
                        self.ack_number = seq_number + 1;

                        // Send ACK for their FIN
                        if let Some(remote) = self.remote_addr {
                            tcp::send_tcp_packet(
                                interface(),
                                self.binding.port,
                                remote.port,
                                self.seq_number,
                                self.ack_number,
                                TCP_FLAG_ACK,
                                self.window_size,
                                remote.addr,
                                Vec::new(),
                            )?;
                        }

                        self.state = TcpState::TimeWait;
                    } else {
                        // Just ACK for our FIN
                        self.state = TcpState::FinWait2;
                    }
                }
            }

            TcpState::FinWait2 => {
                if flags & TCP_FLAG_FIN != 0 {
                    self.ack_number = seq_number + 1;

                    // Send ACK for their FIN
                    if let Some(remote) = self.remote_addr {
                        tcp::send_tcp_packet(
                            interface(),
                            self.binding.port,
                            remote.port,
                            self.seq_number,
                            self.ack_number,
                            TCP_FLAG_ACK,
                            self.window_size,
                            remote.addr,
                            Vec::new(),
                        )?;
                    }

                    self.state = TcpState::TimeWait;
                    // In a real implementation, start the TIME_WAIT timer here
                }
            }

            TcpState::LastAck => {
                if flags & TCP_FLAG_ACK != 0 {
                    // Final ACK received, connection fully closed
                    self.state = TcpState::Closed;
                    self.connected = false;
                    self.remote_addr = None;
                }
            }

            // Handle other states as needed
            _ => {}
        }

        Ok(())
    }

    // Returns the number of packets enqueued for sending.
    pub fn num_send_enqueued(&self) -> usize {
        self.send_buffer.len()
    }

    // Returns the number of packets enqueued for receiving.
    pub fn num_recv_enqueued(&self) -> usize {
        self.recv_buffer.len()
    }

    // Close the connection gracefully
    pub fn close(&mut self) -> Result<()> {
        match self.state {
            TcpState::Established => {
                // Send FIN packet
                if let Some(remote) = self.remote_addr {
                    tcp::send_tcp_packet(
                        interface(),
                        self.binding.port,
                        remote.port,
                        self.seq_number,
                        self.ack_number,
                        TCP_FLAG_FIN | TCP_FLAG_ACK,
                        self.window_size,
                        remote.addr,
                        Vec::new(),
                    )?;

                    self.seq_number += 1; // FIN consumes a sequence number
                    self.state = TcpState::FinWait1;
                }
                Ok(())
            }
            TcpState::CloseWait => {
                if let Some(remote) = self.remote_addr {
                    tcp::send_tcp_packet(
                        interface(),
                        self.binding.port,
                        remote.port,
                        self.seq_number,
                        self.ack_number,
                        TCP_FLAG_FIN | TCP_FLAG_ACK,
                        self.window_size,
                        remote.addr,
                        Vec::new(),
                    )?;

                    self.seq_number += 1;
                    self.state = TcpState::LastAck;
                }
                Ok(())
            }
            _ => Err(Error::Malformed),
        }
    }

    pub fn get_state(&self) -> &TcpState {
        &self.state
    }
}
