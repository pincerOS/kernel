use crate::networking::iface::{ethernet, ipv4, udp, Interface};
use crate::networking::repr::Ipv4Packet;
use crate::networking::socket::{RawSocket, RawType, SocketSet, TaggedSocket, UdpSocket};
use log::warn;

// NOTE: tcp, tcpsocket does not work right now lol
use crate::networking::{Error, Result};
use alloc::vec::Vec;

// try to send out as many socket enqueued packets as possible given Interface
pub fn send(interface: &mut Interface, socket_set: &mut SocketSet) {
    //  round robin style, stop when encountering error per socket
    //  if no sends happen, 1) all busy, 2) all error
    loop {
        let sockets = socket_set.count();
        let mut errors = 0;

        for socket in socket_set.iter_mut() {
            let ok_or_err = match *socket {
                // TaggedSocket::Raw(ref mut socket) => send_raw_socket(interface, socket),
                // TaggedSocket::Tcp(ref mut socket) => send_tcp_socket(interface, socket),
                TaggedSocket::Udp(ref mut socket) => send_udp_socket(interface, socket),
            };

            match ok_or_err {
                Ok(_) => {}
                Err(Error::Device(_err)) => {
                    errors = sockets;
                    break;
                }

                Err(_err) => {
                    errors += 1;
                }
            }
        }

        if errors >= sockets {
            break;
        }
    }
}

// fn send_tcp_socket(interface: &mut Interface, socket: &mut TcpSocket) -> Result<()> {
//     socket.send_dequeue(|ipv4_repr, tcp_repr, payload| {
//         tcp::send_packet(interface, ipv4_repr, tcp_repr, |payload_| {
//             payload_.copy_from_slice(payload);
//         })
//     })
// }

fn send_udp_socket(interface: &mut Interface, socket: &mut UdpSocket) -> Result<()> {
    socket.send_dequeue(|ipv4_repr, udp_repr, payload| {
        udp::send_udp_packet(
            interface,
            ipv4_repr.dst_addr,
            payload.to_vec(),
            udp_repr.src_port,
            udp_repr.dst_port,
        )
    })
}
