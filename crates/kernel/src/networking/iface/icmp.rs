use log::debug;

use crate::networking::repr::{IcmpMessage, IcmpPacket, IcmpRepr, Ipv4Packet, Ipv4Repr};
use crate::networking::iface::{ipv4, Interface};
use crate::networking::{Error, Result};

pub fn recv_icmp_packet(
    interface: &mut Interface,
    ipv4_repr: Ipv4Packet,
) -> Result<()> {
    let icmp_recv_packet = IcmpPacket::deserialize(ipv4_repr.payload)?;
    // icmp_recv_packet.check_encoding()?;

    let (ipv4_repr, icmp_send_repr) = match icmp_recv_packet.message {
        IcmpMessage::EchoRequest { id, seq } => {
            debug!(
                "Got a ping from {}; Sending response...",
                ipv4_repr.src_addr
            );

            let ipv4_send_repr = Ipv4Repr {
                icmp_type: Icmp::
                src_addr: ipv4_repr.dst_addr,
                dst_addr: ipv4_repr.src_addr,
                protocol: ipv4_repr.protocol,
                payload_len: ipv4_repr.payload_len,
            };

            (
                ipv4_send_repr,
                IcmpRepr {
                    message: IcmpMessage::EchoReply { id, seq },
                    payload_len: icmp_recv_repr.payload_len,
                },
            )
        }
        _ => return Err(Error::Ignored),
    };

    send_ip_packet(interface, &ipv4_send_repr,)
}

