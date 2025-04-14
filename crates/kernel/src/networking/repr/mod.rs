/* mainly for representation of different protocols
* 
* allows for modules to have more locally defined fields, and redefines it to allow other protocols
* to be able to access them
*
* sources:
* https://docs.rs/smoltcp/latest/smoltcp/wire/index.html: referenced a lot of their rust setup
* because i was unfamiliar with making a big project like this
* https://github.com/pandax381/microps: c based learning network stack
*
* TODO: smoltcp also does some great error handling, with an Error struct that can even allow for
* recovery. for now, most errors will just panic or will be ignored (print to console or log)
*/

// mod field {
//     pub type Field = ::std::ops::Range<usize>;
//     pub type Rest = ::std::ops::RangeFrom<usize>;
// }

mod ethernet;
mod arp;
mod ipv4;
mod icmp;
mod udp;
// mod dhcp;
pub mod dev;

pub use self::ethernet::{
    EtherType as EthernetType, Address as EthernetAddress, Frame as EthernetFrame,
};

pub use self::arp::{
    Hardware as ArpHardware, Operation as ArpOperation, Packet as ArpPacket
};

pub use self::ipv4::{
    Address as Ipv4Address, AddressCidr as Ipv4Cidr, Packet as Ipv4Packet,
    Protocol as Ipv4Protocol
};

pub use self::icmp::{
    DestinationUnreachable as IcmpDstUnreachable, Message as IcmpMessage, Packet as IcmpPacket,
    TimeExceeded as IcmpTimeExceeded,
};

pub use self::udp::Packet as UdpPacket;

// pub use self::dhcp::{Packet as DHCPPacket, OpCode as DHCPOp};
