/******************************************************************************
*	hcd/dwc/roothub.c
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*	hcd/dwc/roothub.c contains code to control the DesignWare® Hi-Speed USB 2.0
*	On-The-Go (HS OTG) Controller's virtual root hub. The physical USB 
*	connection to the computer is treated as a virtual 1 port USB hub for 
*	simplicity, allowing the USBD to control it directly with a Hub driver.
*
*	THIS SOFTWARE IS NOT AFFILIATED WITH NOR ENDORSED BY SYNOPSYS IP.
******************************************************************************/


use crate::device::usb::usbd::device::*;
use crate::device::usb::types::*;
use crate::device::usb::usbd::request::*;
use crate::device::usb::usbd::pipe::*;
use crate::device::usb::usbd::descriptors::*;
use crate::device::usb::hcd::hub::HubDescriptor;

use crate::device::usb::usbd::usbd::*;
use core::mem::size_of;


pub const RootHubDeviceNumber: usize = 0;

pub fn hcd_process_root_hub_message(
    device: &mut UsbDevice,
    pipe: UsbPipeAddress,
    buffer: *mut u8,
    buffer_length: u32,
    request: &mut UsbDeviceRequest,
) -> ResultCode {
    ResultCode::OK
}



const DeviceDescriptor: UsbDeviceDescriptor = UsbDeviceDescriptor {
    descriptor_length: 0x12,
    descriptor_type: DescriptorType::Device,
    usb_version: 0x0200,
    class: DeviceClass::DeviceClassHub,
    subclass: 0,
    protocol: 0,
    max_packet_size0: 8,
    vendor_id: 0,
    product_id: 0,
    version: 0x0100,
    manufacturer: 0,
    product: 1,
    serial_number: 0,
    configuration_count: 1,
};


#[repr(C, packed)]
struct ConfigurationDescriptor {
    configuration: UsbConfigurationDescriptor,
    interface: UsbInterfaceDescriptor,
    endpoint: UsbEndpointDescriptor,
}

const CONFIGURATION_DESCRIPTOR: ConfigurationDescriptor = ConfigurationDescriptor {
    configuration: UsbConfigurationDescriptor {
        descriptor_length: 9,
        descriptor_type: DescriptorType::Configuration,
        total_length: 0x19,
        interface_count: 1,
        configuration_value: 1,
        string_index: 0,
        attributes: UsbConfigurationAttributes{
            attributes: (1 << 6) | (1 << 7)
        },
        maximum_power: 0,
    },
    interface: UsbInterfaceDescriptor {
        descriptor_length: 9,
        descriptor_type: DescriptorType::Interface,
        number: 0,
        alternate_setting: 0,
        endpoint_count: 1,
        class: InterfaceClass::InterfaceClassHub,
        subclass: 0,
        protocol: 0,
        string_index: 0,
    },
    endpoint: UsbEndpointDescriptor {
        descriptor_length: 7,
        descriptor_type: DescriptorType::Endpoint,
        endpoint_address: UsbEndpointAddress {
            Number: 1 | (1 << 7),
        },
        attributes: UsbEndpointAttributes {
            Type: 3,
        },
        packet: UsbPacket {
            MaxSize: 8,
        },
        interval: 0xff,
    },
};

const STRING_0: UsbStringDescriptor = UsbStringDescriptor {
    descriptor_length: 4,
    descriptor_type: DescriptorType::String,
    data: [0x0409],
};

// const STRING_1: UsbStringDescriptor = UsbStringDescriptor {
//     DescriptorLength: (size_of::<[u16; 16]>() + 2) as u8,
//     DescriptorType: String,
//     Data: &['U' as u16, 'S' as u16, 'B' as u16, ' ' as u16, '2' as u16, '.' as u16, '0' as u16, ' ' as u16, 'R' as u16, 'o' as u16, 'o' as u16, 't' as u16, ' ' as u16, 'H' as u16, 'u' as u16, 'b' as u16],
// };



const HUB_DESCRIPTOR: HubDescriptor = HubDescriptor {
    DescriptorLength: 0x9,
    DescriptorType: DescriptorType::Hub,
    PortCount: 1,
    Attributes: 0,
    PowerGoodDelay: 0,
    MaximumHubPower: 0,
    Data: [0x01, 0xff],
};
