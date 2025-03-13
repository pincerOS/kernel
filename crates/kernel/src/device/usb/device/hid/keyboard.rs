
use super::super::super::usbd::device::*;
use super::super::super::usbd::descriptors::*;
use super::super::super::usbd::usbd::*;

use alloc::vec;
use alloc::boxed::Box;
use crate::device::system_timer::micro_delay;
use crate::device::usb::types::*;
use crate::device::usb::hcd::hub::*;
use crate::device::usb::usbd::pipe::*;
use crate::device::usb::usbd::request::*;


pub fn KbdAttach(device: &mut UsbDevice) {
    
}