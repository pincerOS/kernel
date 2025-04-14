use crate::networking::repr::*;
use crate::networking::iface::{ipv4, Interface};
use crate::networking::socket::{SocketAddr, SocketSet, TaggedSocket};
use crate::networking::{Result, Error};
use alloc::vec::Vec;
use log::debug;

/// Send a TCP packet through the interface
pub fn send_tcp_packet(
    interface: &mut Interface,
    dst_addr: Ipv4Address,
    payload: Vec<u8>,
    src_port: u16,
    dst_port: u16,
    seq_number: u32,
    ack_number: u32,
    flags: u8,
    window_size: u16,
) -> Result<()> {
    debug!("sending tcp {} {} {} {} seq={} ack={} flags={:#04x}", 
           src_port, dst_port, *interface.ipv4_addr, dst_addr, seq_number, ack_number, flags);

    let tcp_packet = TcpPacket::new(
        src_port,
        dst_port,
        seq_number,
        ack_number,
        flags,
        window_size,
        payload,
        *interface.ipv4_addr,
        dst_addr,
    );

    ipv4::send_ipv4_packet(interface, tcp_packet.serialize(), Ipv4Protocol::TCP, dst_addr)
}

/// Process an incoming TCP packet
pub fn recv_tcp_packet(
    interface: &mut Interface,
    ipv4_packet: Ipv4Packet,
) -> Result<()> {
    let tcp_packet = TcpPacket::deserialize(ipv4_packet.payload.as_slice())?;
    
    // Set source and destination IP addresses for validation and checksum verification
    let mut packet_with_ips = tcp_packet.clone();
    packet_with_ips.src_ip = ipv4_packet.src_addr;
    packet_with_ips.dst_ip = ipv4_packet.dst_addr;
    
    // Socket address of the destination
    let dst_socket_addr = SocketAddr {
        addr: ipv4_packet.dst_addr,
        port: tcp_packet.dst_port,
    };

    // Socket address of the source
    let src_socket_addr = SocketAddr {
        addr: ipv4_packet.src_addr,
        port: tcp_packet.src_port,
    };
    
    let mut handled = false;
    
    // Find and process any sockets that should receive this packet
    interface.tcp_sockets
        .iter_mut()
        .filter_map(|socket| match socket {
            TaggedSocket::Tcp(ref mut socket) => {
                if socket.accepts(&dst_socket_addr) {
                    Some(socket)
                } else {
                    None
                }
            }
            _ => None,
        })
        .for_each(|socket| {
            handled = true;
            if let Err(err) = socket.process_packet(&ipv4_packet, &packet_with_ips) {
                debug!(
                    "Error processing TCP packet for socket: {:?}",
                    err
                );
            }
        });
    
    // If no socket handled the packet, check if we need to send a reset
    if !handled {
        // If the packet isn't a reset itself, send a reset
        if (tcp_packet.flags & TCP_RST) == 0 {
            return send_tcp_reset(interface, &packet_with_ips, ipv4_packet.src_addr);
        }
    }
    
    Ok(())
}

/// Handle connection establishment (3-way handshake)
pub fn handle_connection_request(
    interface: &mut Interface,
    peer_addr: SocketAddr,
    local_port: u16,
) -> Result<()> {
    // This would be called when a socket wants to establish a connection
    let seq_number = generate_initial_seq_number();
    
    send_tcp_packet(
        interface,
        peer_addr.addr,
        Vec::new(), // empty payload for SYN
        local_port,
        peer_addr.port,
        seq_number,
        0, // ACK is zero for initial SYN
        TCP_SYN,
        8192, // Default window size
    )
}

/// Close an established connection
pub fn handle_connection_close(
    interface: &mut Interface,
    peer_addr: SocketAddr,
    local_port: u16,
    seq_number: u32,
    ack_number: u32,
) -> Result<()> {
    // Send FIN packet to initiate connection close
    send_tcp_packet(
        interface,
        peer_addr.addr,
        Vec::new(), // empty payload for FIN
        local_port,
        peer_addr.port,
        seq_number,
        ack_number,
        TcpFlags::TCP_FIN | TcpFlags::TCP_ACK,
        8192, // Window size
    )
}

/// Send a TCP reset packet in response to a packet that has no matching socket
fn send_tcp_reset(
    interface: &mut Interface,
    original_packet: &TcpPacket,
    dst_addr: Ipv4Address,
) -> Result<()> {
    let seq_number = if (original_packet.flags & TcpFlags::TCP_ACK) != 0 {
        original_packet.ack_number
    } else {
        0
    };
    
    let ack_number = if (original_packet.flags & (TcpFlags::TCP_SYN | TcpFlags::TCP_FIN)) != 0 {
        original_packet.seq_number.wrapping_add(1)
    } else {
        original_packet.seq_number.wrapping_add(original_packet.payload.len() as u32)
    };
    
    debug!("Sending TCP RST to {}:{}", dst_addr, original_packet.src_port);
    
    send_tcp_packet(
        interface,
        dst_addr,
        Vec::new(), // empty payload for RST
        original_packet.dst_port,
        original_packet.src_port,
        seq_number,
        ack_number,
        TcpFlags::TCP_RST | (if seq_number == 0 { 0 } else { TcpFlags::TCP_ACK }),
        0, // Window size is typically 0 for RST
    )
}

/// Generate a random initial sequence number for new connections
fn generate_initial_seq_number() -> u32 {
    // In a real implementation, this would use a time-based algorithm
    // or a cryptographically secure random number generator.
    // For simplicity, we're using a placeholder.
    1 // In a real impl, replace with proper random number
}

// Retransmit unacknowledged packets based on timeout
pub fn handle_retransmissions(interface: &mut Interface) -> Result<()> {
    // This would be called periodically to check for packets that need retransmission
    interface.tcp_sockets
        .iter_mut()
        .filter_map(|socket| match socket {
            TaggedSocket::Tcp(ref mut socket) => Some(socket),
            _ => None,
        })
        .try_for_each(|socket| {
            socket.process_retransmissions(interface)
        })
}

// Update the TCP state machine based on timeouts
pub fn process_timeouts(interface: &mut Interface) -> Result<()> {
    interface.tcp_sockets
        .iter_mut()
        .filter_map(|socket| match socket {
            TaggedSocket::Tcp(ref mut socket) => Some(socket),
            _ => None,
        })
        .try_for_each(|socket| {
            socket.process_timeouts(interface)
        })
}

/// Utility function to check if a port is in use by any socket
pub fn is_port_in_use(interface: &Interface, port: u16) -> bool {
    interface.tcp_sockets
        .iter_mut()
        .any(|socket| match socket {
            TaggedSocket::Tcp(socket) => socket.local_port() == port,
            _ => false,
        })
}

// Find an unused ephemeral port for outgoing connections
pub fn get_ephemeral_port(interface: &Interface) -> Result<u16> {
    // Standard ephemeral port range
    const PORT_MIN: u16 = 49152;
    const PORT_MAX: u16 = 65535;
    
    for port in PORT_MIN..=PORT_MAX {
        if !is_port_in_use(interface, port) {
            return Ok(port);
        }
    }
    
    Err(Error::NoEphemeralPorts)
}
