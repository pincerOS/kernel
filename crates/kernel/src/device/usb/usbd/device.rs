/******************************************************************************
*	usbd/device.h
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*	usbd/device.h contains a definition of a device structure used for 
*	storing devices and the device tree.
******************************************************************************/

use super::descriptors::*;
use super::pipe::*;
// use super::super::types::*;
use super::super::hcd::dwc::dwc_otg::*;
use super::super::types::{ UsbSpeed, ResultCode };

/// The maximum number of children a device could have, 
/// which is the maximum number of ports a hub supports.
pub const MAX_CHILDREN_PER_DEVICE: usize = 10;

/// The maximum number of interfaces a device configuration could have.
pub const MAX_INTERFACES_PER_DEVICE: usize = 8;

/// The maximum number of endpoints a device could have (per interface).
pub const MAX_ENDPOINTS_PER_DEVICE: usize = 16;

/// Status of a USB device as defined in 9.1 of the USB 2.0 manual.
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UsbDeviceStatus {
    Attached = 0,
    Powered = 1,
    Default = 2,
    Addressed = 3,
    Configured = 4,
}

/// Status of a USB transfer.
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UsbTransferError {
    NoError = 0,
    Stall = 1 << 1,
    BufferError = 1 << 2,
    Babble = 1 << 3,
    NoAcknowledge = 1 << 4,
    CrcError = 1 << 5,
    BitError = 1 << 6,
    ConnectionError = 1 << 7,
    AhbError = 1 << 8,
    NotYetError = 1 << 9,
    Processing = 1 << 31,
}

/// Start of a device-specific data field.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct UsbDriverDataHeader {
    pub device_driver: u32,
    pub data_size: u32,
}

/// Structure to store the details of a detected USB device.
#[repr(C)]
pub struct UsbDevice {
    pub number: u32,
    pub speed: UsbSpeed,
    pub status: UsbDeviceStatus,
    pub configuration_index: u8,
    pub port_number: u8,
    pub error: UsbTransferError,
    
    // Generic device handlers
    pub device_detached: Option<fn(&mut UsbDevice)>,
    pub device_deallocate: Option<fn(&mut UsbDevice)>,
    pub device_check_for_change: Option<fn(&mut UsbDevice)>,
    pub device_child_detached: Option<fn(&mut UsbDevice, &mut UsbDevice)>,
    pub device_child_reset: Option<fn(&mut UsbDevice, &mut UsbDevice) -> Result<(), ()>>,
    pub device_check_connection: Option<fn(&mut UsbDevice, &mut UsbDevice) -> Result<(), ()>>,
    
    pub descriptor: UsbDeviceDescriptor,
    pub configuration: UsbConfigurationDescriptor,
    pub interfaces: [UsbInterfaceDescriptor; MAX_INTERFACES_PER_DEVICE],
    pub endpoints: [[UsbEndpointDescriptor; MAX_ENDPOINTS_PER_DEVICE]; MAX_INTERFACES_PER_DEVICE],
    pub parent: Option<*mut UsbDevice>,
    pub full_configuration: Option<*mut u8>,
    pub driver_data: Option<*mut UsbDriverDataHeader>,
    pub last_transfer: u32,
}

/// Number of interface class attach handlers.
pub const INTERFACE_CLASS_ATTACH_COUNT: usize = 16;

/** The maximum number of devices that can be connected. */
pub const MaximumDevices: usize = 32;

pub struct UsbBus {
    pub devices: [Option<*mut UsbDevice>; MaximumDevices],
    pub interface_class_attach: [Option<fn(device: &mut UsbDevice, interface_number: u32) -> ResultCode>; INTERFACE_CLASS_ATTACH_COUNT],
    pub dwc_sc: dwc_hub
}
