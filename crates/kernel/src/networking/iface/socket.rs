use crate::networking::repr::Ipv4Packet;
use crate::networking::iface::{ethernet, ipv4, udp, Interface};
use crate::networking::socket::{RawSocket, RawType, SocketSet, TaggedSocket, UdpSocket};
use log::warn;

// NOTE: tcp, tcpsocket does not work right now lol
use alloc::vec;
use crate::networking::{Error, Result};

// try to send out as many socket enqueued packets as possible given Interface
pub fn send(interface: &mut Interface, socket_set: &mut SocketSet) {
    //  round robin style, stop when encountering error per socket
    //  if no sends happen, 1) all busy, 2) all error
    loop {
        let sockets = socket_set.count();
        let mut errors = 0;

        for socket in socket_set.iter_mut() {
            let ok_or_err = match *socket {
                TaggedSocket::Raw(ref mut socket) => send_raw_socket(interface, socket),
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

fn send_raw_socket(interface: &mut Interface, socket: &mut RawSocket) -> Result<()> {
    match socket.raw_type() {
        RawType::Ethernet => {
            socket.send_dequeue(|eth_buffer| {
                ethernet::send_frame(interface, eth_buffer.len(), |eth_frame| {
                    eth_frame.as_mut().copy_from_slice(eth_buffer);
                })
            })
        }
        RawType::Ipv4 => socket.send_dequeue(|ipv4_buffer| {
            if let Ok(ipv4_packet) = Ipv4Packet::try_new(ipv4_buffer) {
                ipv4::send_packet_raw(
                    interface,
                    ipv4_packet.dst_addr(),
                    ipv4_buffer.len(),
                    |ipv4_packet| {
                        ipv4_packet.copy_from_slice(ipv4_buffer);
                    },
                )
            } else {
                Ok(())
            }
        }),
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
        udp::send_packet(interface, ipv4_repr, udp_repr, |payload_| {
            payload_.copy_from_slice(payload);
        })
    })
}

// pub fn recv(interface: &mut Interface, socket_set: &mut SocketSet) {
//     let mut eth_buffer = vec![0; 1500];
//
//     loop {
//         let buffer_len = match interface.dev.recv(&mut eth_buffer) {
//             Ok(buffer_len) => buffer_len,
//             Err(Error::Device(_)) => break,
//             Err(_err) => {
//                 break;
//             }
//         };
//
//         // match ethernet::recv_frame(interface, &eth_buffer[.. buffer_len], socket_set) {
//         match ethernet::recv_frame(interface, &eth_buffer[.. buffer_len], buffer_len) {
//             Ok(_) => continue,
//             Err(Error::Ignored) => continue,
//             Err(Error::MacResolution(_)) => continue,
//             Err(err) => warn!("Error processing Ethernet frame with {:?}", err), // TODO: need to
//             // add to error enum
//         }
//     }
// }
