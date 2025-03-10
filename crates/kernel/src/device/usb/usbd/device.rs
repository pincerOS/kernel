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
use alloc::boxed::Box;
// use super::super::types::*;
use super::super::hcd::dwc::dwc_otg::*;
use super::super::types::{ResultCode, UsbSpeed};
use alloc::vec::Vec;

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
#[derive(Default, Debug, Copy, Clone)]
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
    pub device_child_reset: Option<fn(&mut UsbDevice, &mut UsbDevice) -> ResultCode>,
    pub device_check_connection: Option<fn(&mut UsbDevice, &mut UsbDevice) -> ResultCode>,

    pub descriptor: UsbDeviceDescriptor,
    pub configuration: UsbConfigurationDescriptor,
    pub interfaces: [UsbInterfaceDescriptor; MAX_INTERFACES_PER_DEVICE],
    pub endpoints:
        [[UsbEndpointDescriptor; MAX_ENDPOINTS_PER_DEVICE]; MAX_INTERFACES_PER_DEVICE],
    pub parent: Option<*mut UsbDevice>,
    pub full_configuration: Option<Box<[u8]>>, //TODO: the setupfor this is probably very bad
    pub driver_data: Option<Box<[u8]>>, //TODO: the setupfor this is probably very bad
    pub soft_sc: *mut (),
    pub bus: *mut UsbBus,
    pub last_transfer: u32,
}

impl UsbDevice {
    pub fn new(bus: *mut UsbBus, num: u32) -> Self {
        Self {
            number: num,
            speed: UsbSpeed::Low,
            status: UsbDeviceStatus::Attached,
            error: UsbTransferError::NoError,
            port_number: 0,
            configuration_index: 0xff,
            parent: None,

            device_detached: None,
            device_deallocate: None,
            device_check_for_change: None,
            device_child_detached: None,
            device_child_reset: None,
            device_check_connection: None,

            descriptor: UsbDeviceDescriptor::default(),
            configuration: UsbConfigurationDescriptor::default(),
            interfaces: [UsbInterfaceDescriptor::default(); MAX_INTERFACES_PER_DEVICE],
            endpoints: core::array::from_fn(|_| core::array::from_fn(|_| UsbEndpointDescriptor::default())),
            full_configuration: None,
            driver_data: None,
            soft_sc: unsafe { (*bus).dwc_sc.as_mut() as *mut dwc_hub as *mut () },
            bus: bus,
            last_transfer: 0,
        }
    }
}

/// Number of interface class attach handlers.
pub const INTERFACE_CLASS_ATTACH_COUNT: usize = 16;

/** The maximum number of devices that can be connected. */
pub const MaximumDevices: usize = 32;

pub struct UsbBus {
    pub devices: [Option<*mut Box<UsbDevice>>; MaximumDevices],
    pub interface_class_attach: [Option<
        fn(device: &mut UsbDevice, interface_number: u32) -> ResultCode,
    >; INTERFACE_CLASS_ATTACH_COUNT],
    pub roothub_device_number: u32,
    pub dwc_sc: Box<dwc_hub>,
}
