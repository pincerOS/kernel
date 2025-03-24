extern crate byteorder;
extern crate log;

pub mod utils;
pub mod repr;
pub mod socket;
pub mod iface;

use std::io::Error as IOError;
use std::result::Result as StdResult;

use crate::repr::Ipv4Address;
use crate::socket::SocketAddr;

// TODO: make nested errors so we can have more refined structure, right now everything is on the
// same level (e.g. malformed and checksumerror should be of the same main type)
#[derive(Debug)]
pub enum Error {
    // joyce didn't feel like supporting this
    Unsupported,
    // invalid length from method, for invalid length in a packet it will be Malformed
    InvalidLength,
    // max address cannot be resolved to ipv4 address
    MacResolution(Ipv4Address),
    // socket reuse
    BindingInUse(SocketAddr),
    // socket buffer is full or empty
    Exhausted,
    // if you see this, there was an ignored packet, maybe it's bad, maybe i didn't handle the edge
    // case
    Ignored,
    // bad operations on a device (ie. reads from empty ethernet buffer or writes to busy device)
    Device(Option<IOError>),
    // bad packet
    Malformed,
    // bad checksum
    Checksum,
}

pub type Result<T> = StdResult<T, Error>;


