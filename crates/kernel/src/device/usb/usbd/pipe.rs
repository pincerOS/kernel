/******************************************************************************
*	usbd/pipe.h
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*	usbd/pipe.h contains definitions relating to the USB pipe structure,
*	defined as part of the USB protocol. The precise details of this data
*	structure are an implementation detail, matching Linux in this case to
*	aid compatibility.
******************************************************************************/

use super::super::types::*;

/// Our implementation of the USB pipe defined in 10.5.1.
///
/// The `UsbPipeAddress` holds the address of a pipe. The USB standard defines
/// these as a software mechanism for communication between the USB driver and the
/// host controller driver. We shall not have a concept of creating or destroying
/// pipes, as this is needless clutter, and simply indicate the pipe by its
/// physical properties. In other words, we identify the pipe by its physical
/// consequences on the USB. This is similar to Linux and vastly reduces
/// complication, at the expense of requiring a little more sophistication on the
/// sender's behalf.
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct UsbPipeAddress {
    pub max_size: UsbPacketSize,    // 2 bits @0
    pub speed: UsbSpeed,            // 2 bits @2
    pub end_point: u8,              // 4 bits @4
    pub device: u8,                 // 8 bits @8
    pub transfer_type: UsbTransfer, // 2 bits @16
    pub direction: UsbDirection,    // 1 bit @18
    pub _reserved: u16,             // 13 bits @19 (fits within 16 bits)
}

// // Ensure the enums match the bit widths correctly
// #[repr(u8)]
// #[derive(Debug, Copy, Clone)]
// pub enum UsbPacketSize {
//     Size8 = 0,
//     Size16 = 1,
//     Size32 = 2,
//     Size64 = 3,
// }

// #[repr(u8)]
// #[derive(Debug, Copy, Clone)]
// pub enum UsbSpeed {
//     Low = 0,
//     Full = 1,
//     High = 2,
//     Super = 3,
// }

// #[repr(u8)]
// #[derive(Debug, Copy, Clone)]
// pub enum UsbTransfer {
//     Control = 0,
//     Isochronous = 1,
//     Bulk = 2,
//     Interrupt = 3,
// }

// #[repr(u8)]
// #[derive(Debug, Copy, Clone)]
// pub enum UsbDirection {
//     Out = 0,
//     In = 1,
// }
