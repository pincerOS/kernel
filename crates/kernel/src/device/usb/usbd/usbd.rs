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

use super::descriptors::*;
use super::device::*;
use super::pipe::*;
use super::request::*;
use super::super::types::*;
use super::super::configuration::*;


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


pub fn UsbInitialise(bus: &mut UsbBus, base_addr: *mut()) -> ResultCode {
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