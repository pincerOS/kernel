/******************************************************************************
*	device/hub.h
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*	device/hub.h contains definitions relating to the USB hub device.
******************************************************************************/

use crate::device::usb::usbd::descriptors::DescriptorType;
use crate::device::usb::UsbDriverDataHeader;
use crate::device::usb::*;

/**
    \brief The hub descriptor information.
    The hub descriptor structure defined in the USB2.0 manual section
    11.23.2.1.
*/
#[repr(C, packed)]
pub struct HubDescriptor {
    pub DescriptorLength: u8,           // +0x0
    pub DescriptorType: DescriptorType, // +0x1
    pub PortCount: u8,                  // +0x2
    // struct {
    // 	enum HubPortControl {
    // 		Global = 0,
    // 		Individual = 1,
    // 	} PowerSwitchingMode : 2; // @0
    // 	bool Compound : 1; // @2
    // 	enum HubPortControl OverCurrentProtection : 2; // @3
    // 	unsigned ThinkTime : 2; // in +1*8FS units @5
    // 	bool Indicators : 1; // @7
    // 	unsigned _reserved8_15 : 8; // @8
    // } __attribute__ ((__packed__)) Attributes; // +0x3
    pub Attributes: u16,     // +0x3
    pub PowerGoodDelay: u8,  // +0x5
    pub MaximumHubPower: u8, // +0x6
    pub Data: [u8; 2], // +0x7 the data consists of n bytes describing port detatchability, followed by n bytes for compatiblity. n = roundup(ports/8).
}

/**
    \brief Encapsulates the current status of a hub.
    The hub status structure defined in 11.24.2.6 of the USB2.0
    standard.
*/
#[repr(C, packed)]
pub struct HubStatus {
    pub _bitfield: u16,
}

/**
    \brief Encapsulates the change in current status of a hub.
    The hub status change structure defined in 11.24.2.6 of the USB2.0
    standard.
*/
#[repr(C, packed)]
pub struct HubStatusChange {
    pub _bitfield: u16,
}

/**
    \brief Encapsulates the full status of a hub.
    The hub status structure defined in 11.24.2.6 of the USB2.0 standard.
*/
#[repr(C, packed)]
pub struct HubFullStatus {
    pub Status: HubStatus,
    pub Change: HubStatusChange,
}

/**
    \brief Encapsulates the current status of a hub port.
    The hub port status structure defined in 11.24.2.7.1 of the USB2.0
    standard.
*/
#[repr(C, packed)]
pub struct HubPortStatus {
    pub _bitfield: u16,
}

/**
    \brief Encapsulates the change in current status of a hub port.
    The hub port status change structure defined in 11.24.2.7.2 of the USB2.0
    standard.
*/
#[repr(C, packed)]
pub struct HubPortStatusChange {
    pub _bitfield: u16,
}

/**
    \brief Encapsulates the full status of a hub port.
    The hub port status structure defined in 11.24.2.7 of the USB2.0 standard.
*/
#[repr(C, packed)]
pub struct HubPortFullStatus {
    pub Status: HubPortStatus,
    pub Change: HubPortStatusChange,
}

/**
    \brief A feature of a hub port.
    The feautres of a hub port that can be altered.
*/
#[repr(u16)]
pub enum HubPortFeature {
    FeatureConnection = 0,
    FeatureEnable = 1,
    FeatureSuspend = 2,
    FeatureOverCurrent = 3,
    FeatureReset = 4,
    FeaturePower = 8,
    FeatureLowSpeed = 9,
    FeatureHighSpeed = 10,
    FeatureConnectionChange = 16,
    FeatureEnableChange = 17,
    FeatureSuspendChange = 18,
    FeatureOverCurrentChange = 19,
    FeatureResetChange = 20,
}

/** The DeviceDriver field in UsbDriverDataHeader for hubs. */
pub const DeviceDriverHub: u32 = 0x48554230;

/**
    \brief Hub specific data.
    The contents of the driver data field for hubs.
*/
pub struct HubDevice {
    pub Header: UsbDriverDataHeader,
    pub Status: HubFullStatus,
    pub Descriptor: *mut HubDescriptor,
    pub MaxChildren: u32,
    pub PortStatus: [HubPortFullStatus; MAX_CHILDREN_PER_DEVICE],
    pub Children: [*mut UsbDevice; MAX_CHILDREN_PER_DEVICE],
}

/**
    \brief A feature of a hub.
    The feautres of a hub that can be altered.
*/
#[repr(u8)]
pub enum HubFeature {
    FeatureHubPower = 0,
    FeatureHubOverCurrent = 1,
}

#[repr(u8)]
pub enum HubPortControl {
    Global = 0,
    Individual = 1,
}
