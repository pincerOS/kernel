/******************************************************************************
*	configuration.c
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*	configuration.c contains code to load all components. In order to
*	allow the actual source files to be completely independent, one file must
*	exist which depends upon all of them, to perform static initialisation.
*	Each separate 'library' provides a Load method, which ConfigurationLoad
*	simply invoeks all of.
******************************************************************************/

use crate::device::usb::usbd::device::*;
use crate::device::usb::usbd::usbd::*;

use crate::device::usb::device::hub::*;

use super::device::hid::HidLoad;
use crate::device::usb::device::net::NetLoad;

pub fn ConfigurationLoad(bus: &mut UsbBus) {
    UsbLoad(bus);
    // DeviceLoad(bus);
    // PipeLoad(bus);
    // RequestLoad(bus);
    HubLoad(bus);
    HidLoad(bus);
    NetLoad(bus);
}
