/******************************************************************************
*	device/hub.c
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*	device/hub.c contains code relating to the generic USB driver's hubs. USB
*	is designed such that this driver's interface would be virtually the same
*	across all systems, and in fact its implementation varies little either.
******************************************************************************/

use super::super::usbd::device::*;
use super::super::usbd::descriptors::*;

use crate::device::usb::types::*;

pub fn HubLoad(bus: &mut UsbBus) {
    bus.interface_class_attach[InterfaceClass::InterfaceClassHub as usize] = Some(HubAttach);
}

fn HubAttach(device: &mut UsbDevice, interface_number: u32) -> ResultCode {

    println!("HubAttach");

    return ResultCode::OK;

}