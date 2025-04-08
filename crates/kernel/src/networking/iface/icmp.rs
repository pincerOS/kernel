use log::debug;

use crate::networking::repr::{IcmpMessage, IcmpPacket, IcmpRepr, Ipv4Repr};
use crate::networking::iface::{ipv4, Interface};
use crate::networking::{Error, Result};

pub fn send_packet<F>(
    interface: &mut Interface,
    ipv4_repr: &Ipv4Repr,
    icmp_repr: &IcmpRepr,
    f: F,
) -> Result<()>
where
    F: FnOnce(&mut [u8]),
{
    ipv4::send_packet_with_repr(interface, &ipv4_repr, |ipv4_payload| {
        let mut icmp_packet = IcmpPacket::from_bytes(ipv4_payload).unwrap();
        icmp_repr.serialize(&mut icmp_packet).unwrap();
        f(icmp_packet.payload_mut());
        icmp_packet.gen_and_set_checksum();
    })
}

pub fn recv_packet(
    interface: &mut Interface,
    ipv4_repr: &Ipv4Repr,
    icmp_buffer: &[u8],
) -> Result<()> {
    let icmp_recv_packet = IcmpPacket::from_bytes(icmp_buffer)?;
    icmp_recv_packet.check_encoding()?;

    let icmp_recv_repr = IcmpRepr::deserialize(&icmp_recv_packet)?;

    let (ipv4_send_repr, icmp_send_repr) = match icmp_recv_repr.message {
        IcmpMessage::EchoRequest { id, seq } => {
            debug!(
                "Got a ping from {}; Sending response...",
                ipv4_repr.src_addr
            );

            let ipv4_send_repr = Ipv4Repr {
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

    send_packet(interface, &ipv4_send_repr, &icmp_send_repr, |payload| {
        payload.copy_from_slice(icmp_recv_packet.payload());
    })
}

