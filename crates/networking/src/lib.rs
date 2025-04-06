#![no_std]
extern crate byteorder;
extern crate log;
extern crate alloc;

pub mod utils;
pub mod repr;
pub mod socket;
pub mod iface;

// extern crate std;
// use std::io::Error as IOError;
// use std::result::Result as StdResult;

use core::result::Result as CoreResult;

use crate::repr::Ipv4Address;
use crate::socket::SocketAddr;

// TODO: make more detailed
pub struct IOError;

// TODO: make nested errors so we can have more refined structure, right now everything is on the
// same level (e.g. malformed and checksumerror should be of the same main type)
#[derive(Debug)]
pub enum Error {
    // not supported/implemented
    Unsupported,
    InvalidLength,
    // mac address cannot be resolved to ipv4 address
    MacResolution(Ipv4Address),
    // socket reuse
    BindingInUse(SocketAddr),
    // socket buffer is full or empty
    Exhausted,
    Ignored,
    // bad operations on a device (ie. reads from empty ethernet buffer or writes to busy device)
    Device(Option<IOError>),
    // bad packet
    Malformed,
    // bad checksum
    Checksum,
}

pub type Result<T> = CoreResult<T, Error>;


