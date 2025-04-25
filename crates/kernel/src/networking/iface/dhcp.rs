use crate::device::system_timer;

use crate::networking::{Error, Result};
use crate::networking::socket::{SocketAddr, UdpSocket, bind, send_to};
use crate::networking::repr::{
    Ipv4Address, 
    Ipv4Cidr, 
    DhcpPacket, 
    DhcpParam, 
    DhcpOption, 
    DhcpMessageType, 
};
use crate::networking::iface::Interface;

use alloc::vec;
use alloc::vec::Vec;

const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;
const DEFAULT_LEASE_RETRY: usize = 3;
// const DISCOVER_TIMEOUT: u64 = 2;
// const REQUEST_TIMEOUT: u64 = 5;

// Basic DHCP client state machine
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DhcpState {
    Idle,
    Discovering,
    Requesting,
    Bound,
    Renewing,
    Rebinding,
    Released,
}

pub struct Dhcpd {
    state: DhcpState,
    xid: u32,
    retries: usize,
    last_action_time: u64,
    server_identifier: Option<Ipv4Address>,
    offered_ip: Option<Ipv4Address>,
    lease_time: Option<u32>,
    renewal_time: Option<u32>,
    rebind_time: Option<u32>,
    subnet_mask: u32,
    router: Option<Ipv4Address>,
    dns_servers: Vec<Ipv4Address>,
    udp_socket: u16,
}

impl Dhcpd {
    pub fn new() -> Self {
        Self {
            state: DhcpState::Idle,
            xid: 0,
            retries: DEFAULT_LEASE_RETRY,
            // discover_timeout: DISCOVER_TIMEOUT,
            // request_timeout: REQUEST_TIMEOUT,
            last_action_time: 0,
            server_identifier: None,
            offered_ip: None,
            lease_time: None,
            renewal_time: None,
            rebind_time: None,
            subnet_mask: 24,
            router: None,
            dns_servers: Vec::new(),
            udp_socket: 0,
        }
    }

    pub fn is_transacting(&mut self) -> bool {
        self.state == DhcpState::Idle
            || self.state == DhcpState::Discovering
            || self.state == DhcpState::Requesting
    }

    pub fn start(&mut self, interface: &mut Interface) -> Result<()> {
        if self.state != DhcpState::Idle && self.state != DhcpState::Released {
            return Ok(()); 
        }

        self.udp_socket = UdpSocket::new();
        let _ = bind(self.udp_socket,DHCP_CLIENT_PORT);


        let time = system_timer::get_time();
        self.xid = time as u32 ^ 0xDEADBEEF;

        self.state = DhcpState::Discovering;
        self.last_action_time = time;

        send_dhcp_discover(interface, self.udp_socket, self.xid)
    }

    pub fn release(&mut self, interface: &mut Interface) -> Result<()> {
        if self.state != DhcpState::Bound
            && self.state != DhcpState::Renewing
            && self.state != DhcpState::Rebinding
        {
            return Ok(());
        }

        if let (Some(server_id), Some(offered_ip)) = (self.server_identifier, self.offered_ip) {
            let result = send_dhcp_release(interface, self.xid, offered_ip, server_id, self.udp_socket);
            self.state = DhcpState::Released;
            result
        } else {
            Err(Error::Malformed)
        }
    }

    pub fn process_dhcp_packet(
        &mut self,
        interface: &mut Interface,
        packet: DhcpPacket,
    ) -> Result<()> {
        let msg_type = packet.get_message_type().ok_or(Error::Malformed)?;

        match (self.state, msg_type) {
            (DhcpState::Discovering, DhcpMessageType::Offer) => {
                if packet.xid != self.xid {
                    return Err(Error::Ignored);
                }

                // Store offered IP and server identifier
                self.offered_ip = Some(packet.yiaddr);
                self.server_identifier = packet.get_server_identifier();

                println!("DHCP: Received offer for IP {}", packet.yiaddr);

                if let (Some(server_id), Some(offered_ip)) =
                    (self.server_identifier, self.offered_ip)
                {
                    self.state = DhcpState::Requesting;
                    self.last_action_time = system_timer::get_time();
                    self.retries = DEFAULT_LEASE_RETRY;

                    send_dhcp_request(interface, self.xid, offered_ip, server_id, self.udp_socket)?;
                    // send_dhcp_packet_workaround(interface, self.xid, offered_ip, server_id, packet)?;
                } else {
                    return Err(Error::Malformed);
                }
            }
            (DhcpState::Requesting, DhcpMessageType::Ack)
            | (DhcpState::Renewing, DhcpMessageType::Ack)
            | (DhcpState::Rebinding, DhcpMessageType::Ack) => {
                if packet.xid != self.xid {
                    return Err(Error::Ignored);
                }

                // Process lease parameters
                self.lease_time = packet.get_lease_time();

                let sub_mask = packet.get_subnet_mask().unwrap();
                let mask_bytes = sub_mask.as_bytes();
                let mut count = 0;

                for &byte in mask_bytes {
                    let mut b = byte;
                    while b > 0 {
                        count += b & 1;
                        b >>= 1;
                    }
                }
                self.subnet_mask = count as u32;

                self.router = packet.get_router();
                self.dns_servers = packet.get_dns_servers();

                // Calculate renewal and rebinding times
                if let Some(lease_time) = self.lease_time {
                    self.renewal_time = Some(lease_time / 2);
                    self.rebind_time = Some((lease_time * 7) / 8);
                }

                // Update interface IP address
                interface.ipv4_addr = Ipv4Cidr::new(packet.yiaddr, self.subnet_mask).unwrap();

                // Update default gateway if provided
                if let Some(router) = self.router {
                    interface.default_gateway = router;
                }

                self.state = DhcpState::Bound;
                self.last_action_time = system_timer::get_time();

                println!(
                    "\t[+] DHCP: Bound to IP {} with lease time {} seconds on gateway {}",
                    interface.ipv4_addr,
                    self.lease_time.unwrap_or(0),
                    interface.default_gateway,
                );
            }
            (DhcpState::Requesting, DhcpMessageType::Nak)
            | (DhcpState::Renewing, DhcpMessageType::Nak)
            | (DhcpState::Rebinding, DhcpMessageType::Nak) => {
                if packet.xid != self.xid {
                    return Err(Error::Ignored);
                }

                println!("DHCP: Received NAK, restarting discovery");

                // Reset and start over
                self.state = DhcpState::Discovering;
                self.last_action_time = system_timer::get_time();
                self.retries = DEFAULT_LEASE_RETRY;
                self.xid = system_timer::get_time() as u32 ^ 0xEFEF1212;

                send_dhcp_discover(interface, self.udp_socket, self.xid)?;
            }
            _ => {
                // Ignore unexpected messages
                println!(
                    "DHCP: Ignoring message type {:?} in state {:?}",
                    msg_type, self.state
                );
                return Err(Error::Ignored);
            }
        }

        Ok(())
    }
}

pub fn send_dhcp_discover(interface: &mut Interface, socketfd: u16, xid: u32) -> Result<()> {
    println!("DHCP: Sending DISCOVER");

    let packet = DhcpPacket {
        op: 1,    // BOOTREQUEST
        htype: 1, // Ethernet
        hlen: 6,  // MAC address length
        hops: 0,
        xid,
        secs: 0,
        flags: 0x0000,
        ciaddr: Ipv4Address::new([0, 0, 0, 0]),
        yiaddr: Ipv4Address::new([0, 0, 0, 0]),
        siaddr: Ipv4Address::new([0, 0, 0, 0]),
        giaddr: Ipv4Address::new([0, 0, 0, 0]),
        chaddr: interface.ethernet_addr,
        options: vec![
            DhcpOption::message_type(DhcpMessageType::Discover),
            DhcpOption::parameters(vec![
                DhcpParam::SubnetMask,
                DhcpParam::BroadcastAddr,
                DhcpParam::TimeOffset,
                DhcpParam::Router,
                DhcpParam::DomainName,
                DhcpParam::DNS,
                DhcpParam::Hostname,
            ]),
            DhcpOption::end(),
        ],
    };

    send_dhcp_packet(interface, socketfd, &packet)
}

pub fn send_dhcp_request(
    interface: &mut Interface,
    xid: u32,
    requested_ip: Ipv4Address,
    server_id: Ipv4Address,
    socketfd: u16,
) -> Result<()> {
    println!("DHCP: Sending REQUEST for {}", requested_ip);

    let packet = DhcpPacket {
        op: 1,    // BOOTREQUEST
        htype: 1, // Ethernet
        hlen: 6,  // MAC address length
        hops: 0,
        xid,
        secs: 0,
        flags: 0x0000,
        ciaddr: Ipv4Address::new([0, 0, 0, 0]),
        yiaddr: Ipv4Address::new([0, 0, 0, 0]),
        siaddr: Ipv4Address::new([0, 0, 0, 0]),
        giaddr: Ipv4Address::new([0, 0, 0, 0]),
        chaddr: interface.ethernet_addr,
        options: vec![
            DhcpOption::message_type(DhcpMessageType::Request),
            DhcpOption::server_identifier(server_id),
            DhcpOption::requested_ip(requested_ip),
            DhcpOption::parameters(vec![
                DhcpParam::SubnetMask,
                DhcpParam::BroadcastAddr,
                DhcpParam::TimeOffset,
                DhcpParam::Router,
                DhcpParam::DomainName,
                DhcpParam::DNS,
                DhcpParam::Hostname,
            ]),
            DhcpOption::end(),
        ],
    };

    send_dhcp_packet(interface, socketfd, &packet)
}

pub fn send_dhcp_renew(
    interface: &mut Interface,
    xid: u32,
    current_ip: Ipv4Address,
    server_id: Ipv4Address,
    socketfd: u16,
) -> Result<()> {
    println!("DHCP: Sending RENEW for {}", current_ip);

    let packet = DhcpPacket {
        op: 1,    // BOOTREQUEST
        htype: 1, // Ethernet
        hlen: 6,  // MAC address length
        hops: 0,
        xid,
        secs: 0,
        flags: 0,           // No broadcast flag for renew
        ciaddr: current_ip, // Use current IP in ciaddr
        yiaddr: Ipv4Address::new([0, 0, 0, 0]),
        siaddr: Ipv4Address::new([0, 0, 0, 0]),
        giaddr: Ipv4Address::new([0, 0, 0, 0]),
        chaddr: interface.ethernet_addr,
        options: vec![
            DhcpOption::message_type(DhcpMessageType::Request),
            // DhcpOption::server_identifier(server_id),
            DhcpOption::end(),
        ],
    };

    // Send directly to the server rather than broadcast
    send_dhcp_packet_unicast(interface, socketfd, &packet, server_id)
}

pub fn send_dhcp_rebind(
    interface: &mut Interface,
    xid: u32,
    current_ip: Ipv4Address,
    socketfd: u16,
) -> Result<()> {
    println!("DHCP: Sending REBIND for {}", current_ip);

    let packet = DhcpPacket {
        op: 1,    // BOOTREQUEST
        htype: 1, // Ethernet
        hlen: 6,  // MAC address length
        hops: 0,
        xid,
        secs: 0,
        flags: 0x8000,      // Broadcast flag
        ciaddr: current_ip, // Use current IP in ciaddr
        yiaddr: Ipv4Address::new([0, 0, 0, 0]),
        siaddr: Ipv4Address::new([0, 0, 0, 0]),
        giaddr: Ipv4Address::new([0, 0, 0, 0]),
        chaddr: interface.ethernet_addr,
        options: vec![
            DhcpOption::message_type(DhcpMessageType::Request),
            DhcpOption::end(),
        ],
    };

    send_dhcp_packet(interface, socketfd, &packet)
}

pub fn send_dhcp_release(
    interface: &mut Interface,
    xid: u32,
    current_ip: Ipv4Address,
    server_id: Ipv4Address,
    socketfd: u16,
) -> Result<()> {
    println!("DHCP: Sending RELEASE for {}", current_ip);

    let packet = DhcpPacket {
        op: 1,    // BOOTREQUEST
        htype: 1, // Ethernet
        hlen: 6,  // MAC address length
        hops: 0,
        xid,
        secs: 0,
        flags: 0,           // No broadcast flag
        ciaddr: current_ip, // Use current IP in ciaddr
        yiaddr: Ipv4Address::new([0, 0, 0, 0]),
        siaddr: Ipv4Address::new([0, 0, 0, 0]),
        giaddr: Ipv4Address::new([0, 0, 0, 0]),
        chaddr: interface.ethernet_addr,
        options: vec![
            DhcpOption::message_type(DhcpMessageType::Release),
            // DhcpOption::server_identifier(server_id),
            DhcpOption::end(),
        ],
    };

    // Send directly to the server rather than broadcast
    send_dhcp_packet_unicast(interface, socketfd, &packet, server_id)
}

fn send_dhcp_packet(interface: &mut Interface, socketfd: u16, packet: &DhcpPacket) -> Result<()> {
    let data = packet.serialize();
    let saddr = SocketAddr {
        addr: interface.ipv4_addr.broadcast(),
        port: DHCP_SERVER_PORT,
    };

    send_to(socketfd, data, saddr)
}

fn send_dhcp_packet_unicast(
    _interface: &mut Interface,
    socketfd: u16,
    packet: &DhcpPacket,
    server_ip: Ipv4Address,
) -> Result<()> {
    let data = packet.serialize();
    let saddr = SocketAddr {
        addr: server_ip,
        port: DHCP_SERVER_PORT,
    };

    send_to(socketfd, data, saddr)
}
