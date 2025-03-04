/******************************************************************************
*	usbd/usbd.c
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*	usbd.c contains code relating to the generic USB driver. USB
*	is designed such that this driver's interface would be virtually the same
*	across all systems, and in fact its implementation varies little either.
******************************************************************************/

use crate::device::usb::hcd::dwc::dwc_otg::*;
use crate::device::usb::hcd::dwc::roothub::RootHubDeviceNumber;
use alloc::boxed::Box;

use super::super::configuration::*;
use super::super::types::*;
use super::descriptors::*;
use super::device::*;
use super::pipe::*;
use super::request::*;

use core::ptr;

/** The default timeout in ms of control transfers. */
pub const ControlMessageTimeout: usize = 10;

pub fn UsbLoad(bus: &mut UsbBus) {
    for i in 0..MaximumDevices {
        bus.devices[i] = None
    }

    for i in 0..INTERFACE_CLASS_ATTACH_COUNT {
        bus.interface_class_attach[i] = None
    }
}

pub fn UsbInitialise(bus: &mut UsbBus, base_addr: *mut ()) -> ResultCode {
    let mut result = ResultCode::OK;

    ConfigurationLoad(bus);

    if size_of::<UsbDeviceRequest>() != 0x8 {
        println!("Error: UsbDeviceRequest size is not 8 bytes");
        return ResultCode::ErrorCompiler;
    }

    if HcdInitialize(bus, base_addr) != ResultCode::OK {
        println!("Error: HcdInitialize failed");
        return ResultCode::ErrorDevice;
    }

    if HcdStart(bus) != ResultCode::OK {
        println!("Error: HcdStart failed");
        return ResultCode::ErrorDevice;
    }

    result
}

fn UsbControlMessage(
    device: &mut UsbDevice,
    pipe: UsbPipeAddress,
    buffer: &mut [u8],
    request: &mut UsbDeviceRequest,
    timeout: u32,
) -> ResultCode {
    return ResultCode::OK;
}

fn UsbGetDescriptor(
    device: &mut UsbDevice,
    desc_type: DescriptorType,
    index: u8,
    langId: u16,
    buffer: &mut [u8],
    length: u32,
    minimumLength: u32,
    recipient: u8,
) -> ResultCode {
    return ResultCode::OK;
}

fn UsbReadDeviceDescriptor(device: &mut UsbDevice) -> ResultCode {
    if device.speed == UsbSpeed::Low {
    } else if device.speed == UsbSpeed::Full {
    } else {
    }
    return ResultCode::OK;
}

fn UsbAttachDevice(bus: &mut UsbBus, device_number: u32) -> ResultCode {
    let device = bus.devices[device_number as usize]
        .as_mut()
        .unwrap()
        .as_mut();

    let address = device.number;
    device.number = 0;

    return ResultCode::OK;
}

fn UsbAllocateDevice(bus: &mut UsbBus) -> ResultCode {
    for number in 0..MaximumDevices {
        if bus.devices[number].is_none() {
            bus.devices[number] = Some(Box::new(UsbDevice::new((number + 1) as u32)));
            break;
        }
    }

    return ResultCode::OK;
}

fn UsbAttachRootHub(bus: &mut UsbBus) -> ResultCode {
    if bus.devices[RootHubDeviceNumber].is_some() {
        println!("Error: Root hub already attached");
        return ResultCode::ErrorDevice;
    }

    if UsbAllocateDevice(bus) != ResultCode::OK {
        println!("Error: UsbAllocateDevice failed");
        return ResultCode::ErrorMemory;
    }

    bus.devices[RootHubDeviceNumber].as_mut().unwrap().status = UsbDeviceStatus::Powered;

    return UsbAttachDevice(bus, RootHubDeviceNumber as u32);
}
