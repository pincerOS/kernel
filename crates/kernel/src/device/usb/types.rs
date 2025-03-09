/******************************************************************************
*	types.h
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*	types.h contains definitions of standardised types used ubiquitously.
******************************************************************************/

/// Result of a method call.
///
/// Negative results are errors.
/// OK is for a general success.
/// ErrorGeneral is an undisclosed failure.
/// ErrorArgument is a bad input.
/// ErrorRetry is a temporary issue that may disappear, the method should be rerun
/// without modification (the caller is expected to limit number of retries as
/// required).
/// ErrorDevice is a more permanent hardware error (a reset procedure should be
/// enacted before retrying).
/// ErrorIncompatible is a device driver that will not support the detected
/// device.
/// ErrorCompiler is a problem with the configuration of the compiler generating
/// unusable code.
/// ErrorMemory is used when the memory is exhausted.
/// ErrorTimeout is used when a maximum delay is reached when waiting and an
/// operation is unfinished. This does not necessarily mean the operation
/// will not finish, just that it is unreasonably slow.
/// ErrorDisconnected is used when a device is disconnected in transfer.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultCode {
    OK = 0,
    ErrorGeneral = -1,
    ErrorArgument = -2,
    ErrorRetry = -3,
    ErrorDevice = -4,
    ErrorIncompatible = -5,
    ErrorCompiler = -6,
    ErrorMemory = -7,
    ErrorTimeout = -8,
    ErrorDisconnected = -9,
}

/// Direction of USB communication.
///
/// Many and various parts of the USB standard use this 1-bit field to indicate
/// in which direction information flows.
#[repr(u8)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum UsbDirection {
    // HostToDevice = 0,
    #[default]
    Out = 0,
    // DeviceToHost = 1,
    In = 1,
}

impl UsbDirection {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => UsbDirection::Out,
            1 => UsbDirection::In,
            _ => panic!("Invalid value for UsbDirection"),
        }
    }
}

/// Speed of USB communication.
///
/// Many and various parts of the USB standard use this 2-bit field to indicate
/// the speed of communication.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeed {
    High = 0,
    Full = 1,
    Low = 2,
}

impl UsbSpeed {
    /// Converts `UsbSpeed` to a human-readable string representation.
    pub fn to_str(self) -> &'static str {
        match self {
            UsbSpeed::High => "480 Mb/s",
            UsbSpeed::Full => "12 Mb/s",
            UsbSpeed::Low => "1.5 Mb/s",
        }
    }
}

/// Transfer type in USB communication.
///
/// Many and various parts of the USB standard use this 2-bit field to indicate
/// the type of transaction to use.
#[repr(u8)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum UsbTransfer {
    #[default]
    Control = 0,
    Isochronous = 1,
    Bulk = 2,
    Interrupt = 3,
}

impl UsbTransfer {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => UsbTransfer::Control,
            1 => UsbTransfer::Isochronous,
            2 => UsbTransfer::Bulk,
            3 => UsbTransfer::Interrupt,
            _ => panic!("Invalid value for UsbTransfer"),
        }
    }
}

/// Transfer size in USB communication.
///
/// Many and various parts of the USB standard use this 2-bit field to indicate
/// the size of transaction to use.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbPacketSize {
    Bits8,
    Bits16,
    Bits32,
    Bits64,
}

pub const fn size_from_number(size: u32) -> UsbPacketSize {
    if size <= 8 {
        UsbPacketSize::Bits8
    } else if size <= 16 {
        UsbPacketSize::Bits16
    } else if size <= 32 {
        UsbPacketSize::Bits32
    } else {
        UsbPacketSize::Bits64
    }
}

/// Converts `UsbPacketSize` to its numeric representation.
pub fn size_to_number(packet_size: UsbPacketSize) -> u16 {
    match packet_size {
        UsbPacketSize::Bits8 => 8,
        UsbPacketSize::Bits16 => 16,
        UsbPacketSize::Bits32 => 32,
        UsbPacketSize::Bits64 => 64,
    }
}

/// Returns the minimum of two values.
#[macro_export]
macro_rules! min {
    ($x:expr, $y:expr) => {{
        let x = $x;
        let y = $y;
        if x < y {
            x
        } else {
            y
        }
    }};
}

/// Returns the maximum of two values.
#[macro_export]
macro_rules! max {
    ($x:expr, $y:expr) => {{
        let x = $x;
        let y = $y;
        if x > y {
            x
        } else {
            y
        }
    }};
}
