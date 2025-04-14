use crate::networking::iface::{udp, Interface};
use crate::networking::repr::*;
use crate::networking::{Error, Result};
use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;
use log::{debug, info};

const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;
const DEFAULT_LEASE_RETRY: usize = 3;
const DISCOVER_TIMEOUT: u64 = 2;
const REQUEST_TIMEOUT: u64 = 5;

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

pub struct DhcpClient {
    state: DhcpState,
    xid: u32,
    retries: usize,
    discover_timeout: u64,
    request_timeout: u64,
    last_action_time: u64,
    server_identifier: Option<Ipv4Address>,
    offered_ip: Option<Ipv4Address>,
    lease_time: Option<u32>,
    renewal_time: Option<u32>,
    rebind_time: Option<u32>,
    subnet_mask: u32,
    router: Option<Ipv4Address>,
    dns_servers: Vec<Ipv4Address>,
}

impl DhcpClient {
    pub fn new() -> Self {
        Self {
            state: DhcpState::Idle,
            xid: 0,
            retries: DEFAULT_LEASE_RETRY,
            discover_timeout: DISCOVER_TIMEOUT,
            request_timeout: REQUEST_TIMEOUT,
            last_action_time: 0,
            server_identifier: None,
            offered_ip: None,
            lease_time: None,
            renewal_time: None,
            rebind_time: None,
            subnet_mask: 24,
            router: None,
            dns_servers: Vec::new(),
        }
    }

    pub fn start(&mut self, interface: &mut Interface, current_time: u64) -> Result<()> {
        if self.state != DhcpState::Idle && self.state != DhcpState::Released {
            return Ok(());
        }

        // Generate a random transaction ID
        // In a real implementation, you'd use a secure random generator
        self.xid = current_time as u32 ^ 0xABCD1234;

        self.state = DhcpState::Discovering;
        self.last_action_time = current_time;

        send_dhcp_discover(interface, self.xid)
    }

    pub fn release(&mut self, interface: &mut Interface) -> Result<()> {
        if self.state != DhcpState::Bound
            && self.state != DhcpState::Renewing
            && self.state != DhcpState::Rebinding
        {
            return Ok(());
        }

        if let (Some(server_id), Some(offered_ip)) = (self.server_identifier, self.offered_ip) {
            let result = send_dhcp_release(interface, self.xid, offered_ip, server_id);
            self.state = DhcpState::Released;
            result
        } else {
            Err(Error::Malformed)
        }
    }

    pub fn poll(&mut self, interface: &mut Interface, current_time: u64) -> Result<()> {
        match self.state {
            DhcpState::Discovering => {
                if current_time >= self.last_action_time + self.discover_timeout {
                    if self.retries > 0 {
                        self.retries -= 1;
                        self.last_action_time = current_time;
                        send_dhcp_discover(interface, self.xid)?;
                    } else {
                        // Too many retries, give up
                        self.state = DhcpState::Idle;
                        return Err(Error::Timeout);
                    }
                }
            }
            DhcpState::Requesting => {
                if current_time >= self.last_action_time + self.request_timeout {
                    if self.retries > 0 {
                        self.retries -= 1;
                        self.last_action_time = current_time;

                        if let (Some(server_id), Some(offered_ip)) =
                            (self.server_identifier, self.offered_ip)
                        {
                            send_dhcp_request(interface, self.xid, offered_ip, server_id)?;
                        } else {
                            self.state = DhcpState::Idle;
                            return Err(Error::Malformed);
                        }
                    } else {
                        // Too many retries, go back to discovering
                        self.state = DhcpState::Discovering;
                        self.retries = DEFAULT_LEASE_RETRY;
                        self.last_action_time = current_time;
                        send_dhcp_discover(interface, self.xid)?;
                    }
                }
            }
            DhcpState::Bound => {
                // Check if it's time to renew
                let renewal_duration = self.renewal_time;
                if current_time >= self.last_action_time + renewal_duration.unwrap() as u64 {
                    self.state = DhcpState::Renewing;
                    self.last_action_time = current_time;
                    self.retries = DEFAULT_LEASE_RETRY;

                    if let (Some(server_id), Some(offered_ip)) =
                        (self.server_identifier, self.offered_ip)
                    {
                        send_dhcp_renew(interface, self.xid, offered_ip, server_id)?;
                    }
                }
            }
            DhcpState::Renewing => {
                // Check if it's time to rebind
                if let Some(rebind_time) = self.rebind_time {
                    let rebind_duration = rebind_time as u64;
                    if current_time >= self.last_action_time + rebind_duration {
                        self.state = DhcpState::Rebinding;
                        self.last_action_time = current_time;
                        self.retries = DEFAULT_LEASE_RETRY;

                        if let Some(offered_ip) = self.offered_ip {
                            send_dhcp_rebind(interface, self.xid, offered_ip)?;
                        }
                    }
                }
            }
            DhcpState::Rebinding => {
                // Check if lease expired
                let lease_duration = self.lease_time.unwrap() as u64;
                if current_time >= self.last_action_time + lease_duration {
                    // Lease expired, start over
                    self.state = DhcpState::Discovering;
                    self.last_action_time = current_time;
                    self.retries = DEFAULT_LEASE_RETRY;
                    self.xid = current_time as u32 ^ 0xABCD5678;

                    send_dhcp_discover(interface, self.xid)?;
                }
            }
            _ => {} // No action needed for Idle or Released states
        }

        Ok(())
    }

    // TODO: connect this to UDP
    pub fn process_dhcp_packet(
        &mut self,
        interface: &mut Interface,
        packet: DhcpPacket,
        current_time: u64,
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

                debug!("DHCP: Received offer for IP {}", packet.yiaddr);

                if let (Some(server_id), Some(offered_ip)) =
                    (self.server_identifier, self.offered_ip)
                {
                    self.state = DhcpState::Requesting;
                    self.last_action_time = current_time;
                    self.retries = DEFAULT_LEASE_RETRY;

                    send_dhcp_request(interface, self.xid, offered_ip, server_id)?;
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
                self.last_action_time = current_time;

                info!(
                    "DHCP: Bound to IP {} with lease time {} seconds",
                    packet.yiaddr,
                    self.lease_time.unwrap_or(0)
                );
            }
            (DhcpState::Requesting, DhcpMessageType::Nak)
            | (DhcpState::Renewing, DhcpMessageType::Nak)
            | (DhcpState::Rebinding, DhcpMessageType::Nak) => {
                if packet.xid != self.xid {
                    return Err(Error::Ignored);
                }

                debug!("DHCP: Received NAK, restarting discovery");

                // Reset and start over
                self.state = DhcpState::Discovering;
                self.last_action_time = current_time;
                self.retries = DEFAULT_LEASE_RETRY;
                self.xid = current_time as u32 ^ 0xEFEF1212;

                send_dhcp_discover(interface, self.xid)?;
            }
            _ => {
                // Ignore unexpected messages
                debug!(
                    "DHCP: Ignoring message type {:?} in state {:?}",
                    msg_type, self.state
                );
                return Err(Error::Ignored);
            }
        }

        Ok(())
    }
}

pub fn send_dhcp_discover(interface: &mut Interface, xid: u32) -> Result<()> {
    debug!("DHCP: Sending DISCOVER");

    let packet = DhcpPacket {
        op: 1,    // BOOTREQUEST
        htype: 1, // Ethernet
        hlen: 6,  // MAC address length
        hops: 0,
        xid,
        secs: 0,
        flags: 0x8000, // Broadcast flag
        ciaddr: Ipv4Address::new([0, 0, 0, 0]),
        yiaddr: Ipv4Address::new([0, 0, 0, 0]),
        siaddr: Ipv4Address::new([0, 0, 0, 0]),
        giaddr: Ipv4Address::new([0, 0, 0, 0]),
        chaddr: interface.ethernet_addr,
        options: vec![
            DhcpOption::message_type(DhcpMessageType::Discover),
            DhcpOption::end(),
        ],
    };

    send_dhcp_packet(interface, &packet)
}

pub fn send_dhcp_request(
    interface: &mut Interface,
    xid: u32,
    requested_ip: Ipv4Address,
    server_id: Ipv4Address,
) -> Result<()> {
    debug!("DHCP: Sending REQUEST for {}", requested_ip);

    let packet = DhcpPacket {
        op: 1,    // BOOTREQUEST
        htype: 1, // Ethernet
        hlen: 6,  // MAC address length
        hops: 0,
        xid,
        secs: 0,
        flags: 0x8000, // Broadcast flag
        ciaddr: Ipv4Address::new([0, 0, 0, 0]),
        yiaddr: Ipv4Address::new([0, 0, 0, 0]),
        siaddr: Ipv4Address::new([0, 0, 0, 0]),
        giaddr: Ipv4Address::new([0, 0, 0, 0]),
        chaddr: interface.ethernet_addr,
        options: vec![
            DhcpOption::message_type(DhcpMessageType::Request),
            DhcpOption::requested_ip(requested_ip),
            DhcpOption::server_identifier(server_id),
            DhcpOption::end(),
        ],
    };

    send_dhcp_packet(interface, &packet)
}

pub fn send_dhcp_renew(
    interface: &mut Interface,
    xid: u32,
    current_ip: Ipv4Address,
    server_id: Ipv4Address,
) -> Result<()> {
    debug!("DHCP: Sending RENEW for {}", current_ip);

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
            DhcpOption::server_identifier(server_id),
            DhcpOption::end(),
        ],
    };

    // Send directly to the server rather than broadcast
    send_dhcp_packet_unicast(interface, &packet, server_id)
}

pub fn send_dhcp_rebind(
    interface: &mut Interface,
    xid: u32,
    current_ip: Ipv4Address,
) -> Result<()> {
    debug!("DHCP: Sending REBIND for {}", current_ip);

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

    send_dhcp_packet(interface, &packet)
}

pub fn send_dhcp_release(
    interface: &mut Interface,
    xid: u32,
    current_ip: Ipv4Address,
    server_id: Ipv4Address,
) -> Result<()> {
    debug!("DHCP: Sending RELEASE for {}", current_ip);

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
            DhcpOption::server_identifier(server_id),
            DhcpOption::end(),
        ],
    };

    // Send directly to the server rather than broadcast
    send_dhcp_packet_unicast(interface, &packet, server_id)
}

fn send_dhcp_packet(interface: &mut Interface, packet: &DhcpPacket) -> Result<()> {
    println!("woof!!!");
    let data = packet.serialize();
    println!("meow!!!");

    udp::send_udp_packet(
        interface,
        interface.ipv4_addr.broadcast(),
        data,
        DHCP_SERVER_PORT,
        DHCP_CLIENT_PORT,
    )
}

fn send_dhcp_packet_unicast(
    interface: &mut Interface,
    packet: &DhcpPacket,
    server_ip: Ipv4Address,
) -> Result<()> {
    let data = packet.serialize();

    udp::send_udp_packet(
        interface,
        server_ip,
        data,
        DHCP_SERVER_PORT,
        DHCP_CLIENT_PORT,
    )
}
