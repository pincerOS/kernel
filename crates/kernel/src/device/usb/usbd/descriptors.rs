/******************************************************************************
*	usbd/descriptors.h
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*	usbd/descriptors.h contains structures defined in the USB standard that
*	describe various aspects of USB.
******************************************************************************/

use crate::device::usb::types::{UsbDirection, UsbTransfer};

#[repr(u8)]
#[derive(Default, Debug, Clone, Copy)]
pub enum DescriptorType {
    #[default]
    Device = 1,
    Configuration = 2,
    String = 3,
    Interface = 4,
    Endpoint = 5,
    DeviceQualifier = 6,
    OtherSpeedConfiguration = 7,
    InterfacePower = 8,
    Hid = 33,
    HidReport = 34,
    HidPhysical = 35,
    Hub = 41,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct UsbDescriptorHeader {
    pub descriptor_length: u8,
    pub descriptor_type: DescriptorType,
}

#[repr(u8)]
#[derive(Default, Debug, Clone, Copy)]
pub enum DeviceClass {
    DeviceClassInInterface = 0x00,
    DeviceClassCommunications = 0x2,
    #[default]
    DeviceClassHub = 0x9,
    DeviceClassDiagnostic = 0xdc,
    DeviceClassMiscellaneous = 0xef,
    DeviceClassVendorSpecific = 0xff,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct UsbDeviceDescriptor {
    pub descriptor_length: u8,
    pub descriptor_type: DescriptorType,
    pub usb_version: u16,
    pub class: DeviceClass,
    pub subclass: u8,
    pub protocol: u8,
    pub max_packet_size0: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    pub manufacturer: u8,
    pub product: u8,
    pub serial_number: u8,
    pub configuration_count: u8,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct UsbDeviceQualifierDescriptor {
    pub descriptor_length: u8,
    pub descriptor_type: DescriptorType,
    pub usb_version: u16,
    pub class: DeviceClass,
    pub subclass: u8,
    pub protocol: u8,
    pub max_packet_size0: u8,
    pub configuration_count: u8,
    pub _reserved9: u8,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct UsbConfigurationDescriptor {
    pub descriptor_length: u8,
    pub descriptor_type: DescriptorType,
    pub total_length: u16,
    pub interface_count: u8,
    pub configuration_value: u8,
    pub string_index: u8,
    pub attributes: UsbConfigurationAttributes,
    pub maximum_power: u8,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct UsbConfigurationAttributes {
    pub attributes: u8,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct UsbInterfaceDescriptor {
    pub descriptor_length: u8,
    pub descriptor_type: DescriptorType,
    pub number: u8,
    pub alternate_setting: u8,
    pub endpoint_count: u8,
    pub class: InterfaceClass,
    pub subclass: u8,
    pub protocol: u8,
    pub string_index: u8,
}

#[repr(u8)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceClass {
    #[default]
    InterfaceClassReserved = 0x00,
    InterfaceClassAudio = 0x01,
    InterfaceClassCommunications = 0x02,
    InterfaceClassHid = 0x03,
    InterfaceClassPhysical = 0x05,
    InterfaceClassImage = 0x06,
    InterfaceClassPrinter = 0x07,
    InterfaceClassMassStorage = 0x08,
    InterfaceClassHub = 0x09,
    InterfaceClassCdcData = 0x0A,
    InterfaceClassSmartCard = 0x0B,
    InterfaceClassContentSecurity = 0x0D,
    InterfaceClassVideo = 0x0E,
    InterfaceClassPersonalHealthcare = 0x0F,
    InterfaceClassAudioVideo = 0x10,
    InterfaceClassDiagnosticDevice = 0xDC,
    InterfaceClassWirelessController = 0xE0,
    InterfaceClassMiscellaneous = 0xEF,
    InterfaceClassApplicationSpecific = 0xFE,
    InterfaceClassVendorSpecific = 0xFF,
}

impl InterfaceClass {
    pub fn interface_class_to_num(ints: InterfaceClass) -> u8{
        match ints {
            InterfaceClass::InterfaceClassReserved => 0x00,
            InterfaceClass::InterfaceClassAudio => 0x01,
            InterfaceClass::InterfaceClassCommunications => 0x02,
            InterfaceClass::InterfaceClassHid => 0x03,
            InterfaceClass::InterfaceClassPhysical => 0x05,
            InterfaceClass::InterfaceClassImage => 0x06,
            InterfaceClass::InterfaceClassPrinter => 0x07,
            InterfaceClass::InterfaceClassMassStorage => 0x08,
            InterfaceClass::InterfaceClassHub => 0x09,
            InterfaceClass::InterfaceClassCdcData => 0x0A,
            InterfaceClass::InterfaceClassSmartCard => 0x0B,
            InterfaceClass::InterfaceClassContentSecurity => 0x0D,
            InterfaceClass::InterfaceClassVideo => 0x0E,
            InterfaceClass::InterfaceClassPersonalHealthcare => 0x0F,
            InterfaceClass::InterfaceClassAudioVideo => 0x10,
            InterfaceClass::InterfaceClassDiagnosticDevice => 0xDC,
            InterfaceClass::InterfaceClassWirelessController => 0xE0,
            InterfaceClass::InterfaceClassMiscellaneous => 0xEF,
            InterfaceClass::InterfaceClassApplicationSpecific => 0xFE,
            InterfaceClass::InterfaceClassVendorSpecific => 0xFF,
        }
    }
}



#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct UsbEndpointDescriptor {
    pub descriptor_length: u8,
    pub descriptor_type: DescriptorType,
    pub endpoint_address: UsbEndpointAddress,
    pub attributes: UsbEndpointAttributes,
    pub packet: UsbPacket,
    pub interval: u8,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct UsbEndpointAddress {
    //typedef enum {
    // 	HostToDevice = 0,
    // 	Out = 0,
    // 	DeviceToHost = 1,
    // 	In = 1,
    // } UsbDirection;

    // struct {
    //     unsigned Number : 4; // @0
    //     unsigned _reserved4_6 : 3; // @4
    //     UsbDirection Direction : 1; // @7
    // } __attribute__ ((__packed__)) EndpointAddress; // +0x2
    pub Number: u8,
}

pub const fn endpoint_address_to_num(ep: UsbEndpointAddress) -> u8 {
    ep.Number & 0xf
}

pub const fn endpoint_address_to_dir(ep: UsbEndpointAddress) -> UsbDirection {
    UsbDirection::from_u8(ep.Number >> 7)
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct UsbEndpointAttributes {
    // typedef enum {
    //     Control = 0,
    //     Isochronous = 1,
    //     Bulk = 2,
    //     Interrupt = 3,
    // } UsbTransfer;

    //struct {
    // 	UsbTransfer Type : 2; // @0
    // 	enum {
    // 		NoSynchronisation = 0,
    // 		Asynchronous = 1,
    // 		Adaptive = 2,
    // 		Synchrouns = 3,
    // 	} Synchronisation : 2; // @2
    // 	enum {
    // 		Data = 0,
    // 		Feeback = 1,
    // 		ImplicitFeebackData = 2,
    // 	} Usage : 2; // @4
    // 	unsigned _reserved6_7 : 2; // @6
    // } __attribute__ ((__packed__)) Attributes; // +0x3
    pub Type: u8,
}

pub const fn endpoint_attributes_to_type(ep: UsbEndpointAttributes) -> UsbTransfer {
    UsbTransfer::from_u8(ep.Type & 0x3)
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct UsbPacket {
    //struct {
    // 	unsigned MaxSize : 11; // @0
    // 	enum {
    // 		None = 0,
    // 		Extra1 = 1,
    // 		Extra2 = 2,
    // 	} Transactions : 2; // @11
    // 	unsigned _reserved13_15 : 3; // @13
    // } __attribute__ ((__packed__)) Packet; // +0x4
    pub MaxSize: u16,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct UsbStringDescriptor {
    pub descriptor_length: u8,
    pub descriptor_type: DescriptorType,
    pub data: [u16; 1], // Variable-length field
}
