use crate::networking::iface::{ipv4, Interface};
use crate::networking::repr::{
    Ipv4Address,
    Ipv4Protocol,
    Ipv4Packet,
    IcmpMessage,
    IcmpPacket,
};
use crate::networking::{Error, Result};

pub fn send_icmp_packet(
    interface: &mut Interface,
    dst_addr: Ipv4Address,
    message: IcmpMessage,
) -> Result<()> {
    let icmp_packet = IcmpPacket::new(message);

    ipv4::send_ipv4_packet(
        interface,
        icmp_packet.serialize(),
        Ipv4Protocol::ICMP,
        dst_addr,
    )
}

pub fn recv_icmp_packet(interface: &mut Interface, ipv4_packet: Ipv4Packet) -> Result<()> {
    let icmp_recv_packet = IcmpPacket::deserialize(ipv4_packet.payload.as_slice())?;
    // icmp_recv_packet.check_encoding()?;

    let icmp_send_packet = match icmp_recv_packet.message {
        IcmpMessage::EchoRequest { id, seq } => IcmpPacket::new(IcmpMessage::EchoReply { id, seq }),
        _ => return Err(Error::Ignored),
    };

    ipv4::send_ipv4_packet(
        interface,
        icmp_send_packet.serialize(),
        Ipv4Protocol::ICMP,
        ipv4_packet.src_addr,
    )
}
