use crate::networking::repr::{Ipv4Protocol, Ipv4Packet, Packet};
use crate::networking::socket::{SocketAddr, SocketAddrLease};
use crate::networking::utils::{ring::Ring, slice::Slice};
use crate::networking::{Error, Result};
use crate::networking::iface::{Interface, tcp};
use core::time::Duration;
use alloc::vec::Vec;

/// TCP connection states based on RFC 793
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

/// Structure holding information about a TCP connection
pub struct TcpConnection {
    // Connection state
    pub state: TcpState,
    // Remote address
    pub remote_addr: SocketAddr,
    // Local sequence number
    pub local_seq: u32,
    // Last acknowledged sequence number from remote host
    pub local_ack: u32,
    // Remote sequence number
    pub remote_seq: u32,
    // Window size
    pub window_size: u16,
    // Timeout values
    pub retransmit_timeout: Duration,
    pub last_activity: u64,
}

/// A TCP socket
pub struct TcpSocket {
    binding: SocketAddrLease,
    send_buffer: Ring<(Slice<u8>, SocketAddr)>,
    recv_buffer: Ring<(Slice<u8>, SocketAddr)>,
    // Maximum segment size
    mss: u16,
    // Connection state
    connection: Option<TcpConnection>,
    // Unacknowledged segments
    unacked_segments: Ring<(Slice<u8>, u32, u64)>, // (data, seq, timestamp)
    // Backlog for listening sockets
    backlog: Ring<TcpConnection>,
    // Maximum backlog size
    backlog_size: usize,
    // Number of retransmission attempts
    retransmit_count: u8,
    // Current timestamp (in milliseconds)
    current_time: u64,
}

impl TcpSocket {
    pub fn new(
        binding: SocketAddrLease,
        send_buffer: Ring<(Slice<u8>, SocketAddr)>,
        recv_buffer: Ring<(Slice<u8>, SocketAddr)>,
        mss: u16,
        backlog_size: usize,
    ) -> TcpSocket {
        TcpSocket {
            binding,
            send_buffer,
            recv_buffer,
            mss,
            connection: None,
            unacked_segments: Ring::new(16), // Reasonable default buffer size
            backlog: Ring::new(backlog_size),
            backlog_size,
            retransmit_count: 0,
            current_time: 0,
        }
    }

    /// Return the local port bound to this socket
    pub fn local_port(&self) -> u16 {
        self.binding.port
    }

    /// Return the socket's current state
    pub fn state(&self) -> TcpState {
        match &self.connection {
            Some(conn) => conn.state,
            None => TcpState::Closed,
        }
    }

    /// Set the socket to listening state
    pub fn listen(&mut self) -> Result<()> {
        if self.connection.is_some() {
            return Err(Error::InUse);
        }
        
        // No actual connection when in listen state
        self.connection = None;
        
        Ok(())
    }

    /// Initiate a connection to a remote host
    pub fn connect(&mut self, remote_addr: SocketAddr) -> Result<()> {
        if self.connection.is_some() {
            return Err(Error::InUse);
        }
        
        // Generate initial sequence number
        let seq = self.generate_isn();
        
        // Initialize connection in SYN_SENT state
        self.connection = Some(TcpConnection {
            state: TcpState::SynSent,
            remote_addr,
            local_seq: seq,
            local_ack: 0,
            remote_seq: 0,
            window_size: 16384, // Default window size
            retransmit_timeout: Duration::from_millis(200), // Default timeout
            last_activity: self.current_time,
        });
        
        Ok(())
    }

    /// Close the connection
    pub fn close(&mut self) -> Result<()> {
        match &mut self.connection {
            Some(conn) => {
                match conn.state {
                    TcpState::Established => {
                        conn.state = TcpState::FinWait1;
                        Ok(())
                    },
                    TcpState::CloseWait => {
                        conn.state = TcpState::LastAck;
                        Ok(())
                    },
                    TcpState::Closed | TcpState::Listen => {
                        self.connection = None;
                        Ok(())
                    },
                    _ => Err(Error::InvalidState),
                }
            },
            None => Ok(()),
        }
    }

    /// Send data over the connection
    pub fn send(&mut self, buffer_len: usize, addr: SocketAddr) -> Result<&mut [u8]> {
        match &self.connection {
            Some(conn) if conn.state == TcpState::Established => {
                if addr != conn.remote_addr {
                    return Err(Error::Illegal);
                }
            },
            Some(_) => return Err(Error::InvalidState),
            None => return Err(Error::Illegal),
        }
        
        self.send_buffer
            .enqueue_maybe(|&mut (ref mut buffer, ref mut addr_)| {
                buffer.try_resize(buffer_len, 0)?;
                for i in 0..buffer_len {
                    buffer[i] = 0;
                }
                *addr_ = addr;
                Ok(&mut buffer[..buffer_len])
            })
    }

    /// Receive data from the connection
    pub fn recv(&mut self) -> Result<(&[u8], SocketAddr)> {
        match &self.connection {
            Some(conn) if conn.state == TcpState::Established || conn.state == TcpState::CloseWait => {
                self.recv_buffer
                    .dequeue_with(|&mut (ref buffer, ref addr)| (&buffer[..], addr.clone()))
            },
            _ => Err(Error::InvalidState),
        }
    }

    /// Dequeue and process a packet for sending
    pub fn send_dequeue<F, R>(&mut self, f: F) -> Result<R>
    where
        F: FnOnce(&Ipv4Packet, &Packet, &[u8]) -> Result<R>,
    {
        let binding = self.binding.clone();
        
        match &mut self.connection {
            Some(conn) => {
                let remote_addr = conn.remote_addr;
                let local_seq = conn.local_seq;
                let remote_seq = conn.remote_seq;
                
                match conn.state {
                    TcpState::SynSent => {
                        // Send SYN packet
                        let tcp_packet = Packet::new(
                            binding.port,
                            remote_addr.port,
                            local_seq,
                            0, // No ACK yet
                            tcp::TCP_SYN,
                            conn.window_size,
                            Vec::new(),
                            binding.addr,
                            remote_addr.addr,
                        );
                        
                        let ipv4_packet = Ipv4Packet::new(
                            binding.addr,
                            remote_addr.addr,
                            Ipv4Protocol::TCP,
                            tcp_packet.serialize(),
                        );
                        
                        // Update connection state
                        conn.last_activity = self.current_time;
                        
                        // Record packet for potential retransmission
                        let mut syn_data = Slice::new(Vec::with_capacity(0));
                        self.unacked_segments.enqueue((syn_data, local_seq, self.current_time))?;
                        
                        f(&ipv4_packet, &tcp_packet, &[])
                    },
                    TcpState::Established => {
                        return self.send_buffer
                            .dequeue_maybe(|&mut (ref mut buffer, addr)| {
                                let flags = tcp::TCP_ACK | tcp::TCP_PSH;
                                
                                let tcp_packet = Packet::new(
                                    binding.port,
                                    addr.port,
                                    local_seq,
                                    remote_seq,
                                    flags,
                                    conn.window_size,
                                    buffer.to_vec(),
                                    binding.addr,
                                    addr.addr,
                                );
                                
                                let ipv4_packet = Ipv4Packet::new(
                                    binding.addr,
                                    addr.addr,
                                    Ipv4Protocol::TCP,
                                    tcp_packet.serialize(),
                                );
                                
                                // Record for potential retransmission
                                let data_len = buffer.len();
                                if data_len > 0 {
                                    let mut data_slice = Slice::new(Vec::with_capacity(data_len));
                                    data_slice.copy_from_slice(&buffer[..]);
                                    self.unacked_segments.enqueue((data_slice, local_seq, self.current_time))?;
                                }
                                
                                // Update sequence number
                                conn.local_seq = conn.local_seq.wrapping_add(data_len as u32);
                                conn.last_activity = self.current_time;
                                
                                f(&ipv4_packet, &tcp_packet, &buffer[..])
                            });
                    },
                    TcpState::FinWait1 | TcpState::LastAck => {
                        // Send FIN packet
                        let tcp_packet = Packet::new(
                            binding.port,
                            remote_addr.port,
                            local_seq,
                            remote_seq,
                            tcp::TCP_FIN | tcp::TCP_ACK,
                            conn.window_size,
                            Vec::new(),
                            binding.addr,
                            remote_addr.addr,
                        );
                        
                        let ipv4_packet = Ipv4Packet::new(
                            binding.addr,
                            remote_addr.addr,
                            Ipv4Protocol::TCP,
                            tcp_packet.serialize(),
                        );
                        
                        // Update connection state and sequence numbers
                        conn.local_seq = conn.local_seq.wrapping_add(1); // FIN consumes a sequence number
                        conn.last_activity = self.current_time;
                        
                        f(&ipv4_packet, &tcp_packet, &[])
                    },
                    _ => Err(Error::InvalidState),
                }
            },
            None => Err(Error::Illegal),
        }
    }

    /// Process an incoming TCP packet
    pub fn process_packet(
        &mut self,
        ipv4_repr: &Ipv4Packet,
        tcp_repr: &Packet,
    ) -> Result<()> {
        let src_addr = SocketAddr {
            addr: ipv4_repr.src_addr,
            port: tcp_repr.src_port,
        };
        
        match &mut self.connection {
            Some(conn) => {
                // Validate that packet is for this connection
                if src_addr != conn.remote_addr {
                    return Err(Error::Ignored);
                }
                
                // Process packet based on connection state
                match conn.state {
                    TcpState::SynSent => {
                        if (tcp_repr.flags & tcp::TCP_SYN) != 0 && (tcp_repr.flags & tcp::TCP_ACK) != 0 {
                            // Received SYN-ACK
                            conn.remote_seq = tcp_repr.seq_number.wrapping_add(1);
                            conn.local_ack = tcp_repr.seq_number.wrapping_add(1);
                            conn.local_seq = tcp_repr.ack_number;
                            conn.state = TcpState::Established;
                            conn.last_activity = self.current_time;
                            
                            // Clear retransmission queue
                            self.unacked_segments.clear();
                            self.retransmit_count = 0;
                            
                            Ok(())
                        } else if (tcp_repr.flags & tcp::TCP_RST) != 0 {
                            // Connection rejected
                            self.connection = None;
                            Err(Error::ConnectionRefused)
                        } else {
                            Err(Error::Ignored)
                        }
                    },
                    TcpState::Established => {
                        if (tcp_repr.flags & tcp::TCP_RST) != 0 {
                            // Connection reset by peer
                            self.connection = None;
                            return Err(Error::ConnectionReset);
                        }
                        
                        if (tcp_repr.flags & tcp::TCP_FIN) != 0 {
                            // Peer wants to close the connection
                            conn.remote_seq = tcp_repr.seq_number.wrapping_add(1);
                            conn.local_ack = tcp_repr.seq_number.wrapping_add(1);
                            conn.state = TcpState::CloseWait;
                            conn.last_activity = self.current_time;
                        }
                        
                        // Process data if present
                        if !tcp_repr.payload.is_empty() {
                            if tcp_repr.seq_number == conn.local_ack {
                                // In-order packet, process it
                                self.recv_buffer
                                    .enqueue_maybe(|&mut (ref mut buffer, ref mut addr)| {
                                        buffer.try_resize(tcp_repr.payload.len(), 0)?;
                                        buffer.copy_from_slice(&tcp_repr.payload);
                                        *addr = src_addr;
                                        
                                        // Update sequence numbers
                                        conn.local_ack = conn.local_ack.wrapping_add(tcp_repr.payload.len() as u32);
                                        
                                        Ok(())
                                    })?;
                            }
                            // Else out-of-order packet, we should buffer it but that's complex
                        }
                        
                        // Process ACKs
                        if (tcp_repr.flags & tcp::TCP_ACK) != 0 {
                            // Remove acknowledged data from retransmission queue
                            self.process_acks(tcp_repr.ack_number);
                        }
                        
                        conn.last_activity = self.current_time;
                        Ok(())
                    },
                    TcpState::FinWait1 => {
                        if (tcp_repr.flags & tcp::TCP_ACK) != 0 && 
                            tcp_repr.ack_number == conn.local_seq {
                            // Our FIN was acknowledged
                            conn.state = TcpState::FinWait2;
                            conn.last_activity = self.current_time;
                        }
                        
                        if (tcp_repr.flags & tcp::TCP_FIN) != 0 {
                            // Peer also sent FIN (simultaneous close)
                            conn.remote_seq = tcp_repr.seq_number.wrapping_add(1);
                            conn.local_ack = tcp_repr.seq_number.wrapping_add(1);
                            
                            if conn.state == TcpState::FinWait2 {
                                // Both sides' FINs acknowledged, move to TIME_WAIT
                                conn.state = TcpState::TimeWait;
                            } else {
                                // Our FIN not yet acknowledged, move to CLOSING
                                conn.state = TcpState::Closing;
                            }
                            conn.last_activity = self.current_time;
                        }
                        
                        Ok(())
                    },
                    TcpState::FinWait2 => {
                        if (tcp_repr.flags & tcp::TCP_FIN) != 0 {
                            // Received FIN from peer
                            conn.remote_seq = tcp_repr.seq_number.wrapping_add(1);
                            conn.local_ack = tcp_repr.seq_number.wrapping_add(1);
                            conn.state = TcpState::TimeWait;
                            conn.last_activity = self.current_time;
                        }
                        
                        Ok(())
                    },
                    TcpState::Closing => {
                        if (tcp_repr.flags & tcp::TCP_ACK) != 0 && 
                            tcp_repr.ack_number == conn.local_seq {
                            // Our FIN was acknowledged
                            conn.state = TcpState::TimeWait;
                            conn.last_activity = self.current_time;
                        }
                        
                        Ok(())
                    },
                    TcpState::LastAck => {
                        if (tcp_repr.flags & tcp::TCP_ACK) != 0 && 
                            tcp_repr.ack_number == conn.local_seq {
                            // Our FIN was acknowledged, connection fully closed
                            self.connection = None;
                        }
                        
                        Ok(())
                    },
                    TcpState::TimeWait => {
                        // Just acknowledge any retransmissions
                        conn.last_activity = self.current_time;
                        Ok(())
                    },
                    _ => Err(Error::InvalidState),
                }
            },
            None => {
                // No established connection, check if this is a new SYN
                if (tcp_repr.flags & tcp::TCP_SYN) != 0 && (tcp_repr.flags & tcp::TCP_ACK) == 0 {
                    // This is a SYN packet, create new connection in SYN_RECEIVED state
                    let conn = TcpConnection {
                        state: TcpState::SynReceived,
                        remote_addr: src_addr,
                        local_seq: self.generate_isn(),
                        local_ack: tcp_repr.seq_number.wrapping_add(1),
                        remote_seq: tcp_repr.seq_number.wrapping_add(1),
                        window_size: 16384, // Default window size
                        retransmit_timeout: Duration::from_millis(200), // Default timeout
                        last_activity: self.current_time,
                    };
                    
                    // For a listening socket, add to backlog
                    if self.backlog.len() < self.backlog_size {
                        self.backlog.enqueue(conn)?;
                        Ok(())
                    } else {
                        Err(Error::Overflow)
                    }
                } else {
                    Err(Error::Ignored)
                }
            },
        }
    }

    /// Check if this socket is bound to the specified address
    pub fn accepts(&self, dst_addr: &SocketAddr) -> bool {
        &(*self.binding) == dst_addr
    }

    /// Process timeouts for this socket
    pub fn process_timeouts(&mut self, interface: &mut Interface) -> Result<()> {
        let binding = self.binding.clone();
        
        match &mut self.connection {
            Some(conn) => {
                // Check for connection timeout
                let elapsed = self.current_time - conn.last_activity;
                let timeout_ms = conn.retransmit_timeout.as_millis() as u64;
                
                match conn.state {
                    TcpState::SynSent => {
                        if elapsed > timeout_ms {
                            self.retransmit_count += 1;
                            if self.retransmit_count > 5 {
                                // Connection attempt failed
                                self.connection = None;
                                return Err(Error::Timeout);
                            }
                            
                            // Retransmit SYN
                            let remote_addr = conn.remote_addr;
                            tcp::send_tcp_packet(
                                interface,
                                remote_addr.addr,
                                Vec::new(),
                                binding.port,
                                remote_addr.port,
                                conn.local_seq,
                                0,
                                tcp::TCP_SYN,
                                conn.window_size,
                            )?;
                            
                            conn.last_activity = self.current_time;
                        }
                    },
                    TcpState::TimeWait => {
                        // After 2 * MSL (Maximum Segment Lifetime), close the connection
                        if elapsed > 2 * 60 * 1000 { // 2 minutes, typically 2*MSL is 2*60s
                            self.connection = None;
                        }
                    },
                    _ => {
                        // Check for idle timeout in established connection
                        if elapsed > 5 * 60 * 1000 { // 5 minutes
                            // Send RST to forcibly close
                            let remote_addr = conn.remote_addr;
                            tcp::send_tcp_packet(
                                interface,
                                remote_addr.addr,
                                Vec::new(),
                                binding.port,
                                remote_addr.port,
                                conn.local_seq,
                                conn.remote_seq,
                                tcp::TCP_RST | tcp::TCP_ACK,
                                0,
                            )?;
                            
                            // Close connection
                            self.connection = None;
                        }
                    },
                }
                
                Ok(())
            },
            None => Ok(()),
        }
    }

    /// Process and handle packet retransmissions
    pub fn process_retransmissions(&mut self, interface: &mut Interface) -> Result<()> {
        let binding = self.binding.clone();
        
        match &mut self.connection {
            Some(conn) => {
                if conn.state != TcpState::Established {
                    return Ok(());
                }
                
                let remote_addr = conn.remote_addr;
                let timeout_ms = conn.retransmit_timeout.as_millis() as u64;
                
                // Check unacked segments for retransmission
                for (data, seq, timestamp) in self.unacked_segments.iter_mut() {
                    if self.current_time - *timestamp > timeout_ms {
                        // Retransmit this segment
                        tcp::send_tcp_packet(
                            interface,
                            remote_addr.addr,
                            data.to_vec(),
                            binding.port,
                            remote_addr.port,
                            *seq,
                            conn.remote_seq,
                            tcp::TCP_ACK | tcp::TCP_PSH,
                            conn.window_size,
                        )?;
                        
                        // Update timestamp for next retransmission
                        *timestamp = self.current_time;
                        
                        // Exponential backoff for retransmission timeout
                        conn.retransmit_timeout = Duration::from_millis(
                            (conn.retransmit_timeout.as_millis() * 2).min(30000) as u64
                        );
                    }
                }
                
                Ok(())
            },
            None => Ok(()),
        }
    }

    /// Update the current timestamp used for timeouts
    pub fn update_timestamp(&mut self, timestamp_ms: u64) {
        self.current_time = timestamp_ms;
    }
    
    /// Generate an initial sequence number
    fn generate_isn(&self) -> u32 {
        // In a real implementation, this would use a time-based algorithm
        // or a cryptographically secure random number generator
        self.current_time as u32 & 0xFFFFFFFF
    }
    
    /// Accept a pending connection from the backlog
    pub fn accept(&mut self) -> Result<SocketAddr> {
        if self.connection.is_some() {
            return Err(Error::InUse);
        }
        
        match self.backlog.dequeue() {
            Ok(conn) => {
                self.connection = Some(conn);
                Ok(conn.remote_addr)
            },
            Err(e) => Err(e),
        }
    }
    
    /// Process received ACKs and remove acknowledged data from retransmission queue
    fn process_acks(&mut self, ack_number: u32) {
        let mut i = 0;
        while i < self.unacked_segments.len() {
            let (_, seq, _) = &self.unacked_segments[i];
            if self.is_seq_acked(*seq, ack_number) {
                // This segment is acknowledged, remove it
                self.unacked_segments.remove(i);
            } else {
                i += 1;
            }
        }
        
        // Reset retransmission timeout on successful ACK
        if let Some(conn) = &mut self.connection {
            conn.retransmit_timeout = Duration::from_millis(200); // Reset to default
            self.retransmit_count = 0;
        }
    }
    
    /// Check if a sequence number is acknowledged by the given ACK
    fn is_seq_acked(&self, seq: u32, ack: u32) -> bool {
        // Handle sequence number wraparound
        if seq < ack {
            ack - seq <= 0x7FFFFFFF
        } else {
            seq - ack > 0x7FFFFFFF
        }
    }
    
    /// Returns the number of packets enqueued for sending
    pub fn send_enqueued(&self) -> usize {
        self.send_buffer.len()
    }

    /// Returns the number of packets enqueued for receiving
    pub fn recv_enqueued(&self) -> usize {
        self.recv_buffer.len()
    }
    
    /// Returns the maximum segment size for this socket
    pub fn mss(&self) -> u16 {
        self.mss
    }
}
