/*-
 * SPDX-License-Identifier: BSD-2-Clause
 *
 * Copyright (c) 2009 Andrew Thompson
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR ``AS IS'' AND ANY EXPRESS OR
 * IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 * OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
 * IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT, INDIRECT,
 * INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
 * NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
 * THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
 * THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */


use core::error;

use crate::{device::dwc_otg::dwc_otg_roothub_exec, shutdown, SpinLock};
use crate::device::dwc_otg::*;
use crate::device::usbreg::*;

pub fn uhub_attach(sc: &mut dwc_otg_softc)  {
    /* assuming that there is one port */
    let req = usb_device_request {
        bmRequestType: UT_READ_CLASS_OTHER,
        bRequest: UR_GET_STATUS,
        wValue: 0,
        wIndex: 1,
        wLength: (8 + 8 * 8) / 8,
    };

    let (error, ptr, len) = dwc_otg_roothub_exec(sc, req);

    println!("| USB: uhub_attach: error: {:?}, ptr: {:?}, len: {:?}", error, ptr, len);
    unsafe { println!("| USB: uhub_attach: sc_hub_temp wValue: {:?}", sc.sc_hub_temp.wValue); }
    unsafe { println!("| USB: uhub_attach: sc_hub_temp usb_hub_descriptor_min: {:?}", sc.sc_hub_temp.ps); }

    println!("Enable power on port");
    let mut wValue: u16 = 0;
    usetw2(&mut wValue, UHF_PORT_POWER as u8, 0);
    let req = usb_device_request {
        bmRequestType: UT_WRITE_CLASS_OTHER,
        bRequest: UR_SET_FEATURE,
        wValue: UHF_PORT_POWER,
        wIndex: 1,
        wLength: 0,
    };

    let (error, ptr, len) = dwc_otg_roothub_exec(sc, req);
    println!("| USB: power uhub_attach: error: {:?}, ptr: {:?}, len: {:?}", error, ptr, len);
    unsafe { println!("| USB: power uhub_attach: sc_hub_temp wValue: {:?}", sc.sc_hub_temp.wValue); }
    unsafe { println!("| USB: power uhub_attach: sc_hub_temp usb_hub_descriptor_min: {:?}", sc.sc_hub_temp.ps); }
}


//https://elixir.bootlin.com/freebsd/v14.2/source/sys/dev/usb/usb_hub.c#L957
pub fn uhub_root_intr() {
    println!("| USB: uhub_root_intr");
    println!("| FUnction not implemented");

    let mut sc = unsafe { &mut *dwc_otg_sc };

    uhub_attach(sc);

    let req = usb_device_request {

        bmRequestType: UT_READ_CLASS_OTHER,
        bRequest: UR_GET_STATUS,
        wValue: 0,
        wIndex: 1,
        wLength: 0,
    };
    let (error, ptr, len) = dwc_otg_roothub_exec(sc, req);

    println!("| USB: get uhub_root_intr: error: {:?}, ptr: {:?}, len: {:?}", error, ptr, len);

    //print sc_hub_temp
    unsafe { println!("| USB: get uhub_root_intr: sc_hub_temp wValue: {:?}", sc.sc_hub_temp.wValue); }
    unsafe { println!("| USB:  get uhub_root_intr: sc_hub_temp usb_port_status: {:?}", sc.sc_hub_temp.ps); }

    let port = unsafe { sc.sc_hub_temp.ps.wPortStatus };
    if port == 256 {
        let req = usb_device_request {

            bmRequestType: UT_WRITE_CLASS_OTHER,
            bRequest: UR_SET_FEATURE,
            wValue: UHF_PORT_RESET,
            wIndex: 1,
            wLength: 0,
        };
        let (error, ptr, len) = dwc_otg_roothub_exec(sc, req);
        println!("| USB: reset uhub_root_intr: error: {:?}, ptr: {:?}, len: {:?}", error, ptr, len);
    
        //print sc_hub_temp
        unsafe { println!("| USB: reset uhub_root_intr: sc_hub_temp wValue: {:?}", sc.sc_hub_temp.wValue); }
        unsafe { println!("| USB: reset uhub_root_intr: sc_hub_temp usb_port_status: {:?}", sc.sc_hub_temp.ps); }
    }

    

    //not too sure what this does, TODO: implement
}

pub fn usb_callout_reset() {
    println!("| USB: usb_callout_reset");
    println!("| FUnction not implemented");
    //not too sure what this does, TODO: implement
}

pub fn usb_callout_stop() {
    println!("| USB: usb_callout_stop");
    println!("| FUnction not implemented");
    //not too sure what this does, TODO: implement
}

pub fn usetw2(w: &mut u16, v: u8, v2: u8) {
    *w = ((v2 as u16) << 8) | (v as u16);
}

pub fn usetw(w: &mut u16, v: u16) {
    // w[0] = v as u8;
    // w[1] = (v >> 8) as u8;
    *w = v;
}

pub fn usetw_lower(w: &mut u16, v: u8) {
    *w = (*w & 0xff00) | v as u16;
}


#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)] 
pub enum usb_error_t { /* keep in sync with usb_errstr_table */
    //usb/usbdi.h
    USB_ERR_NORMAL_COMPLETION = 0,
    USB_ERR_PENDING_REQUESTS,   // 1
    USB_ERR_NOT_STARTED,        // 2
    USB_ERR_INVAL,              // 3
    USB_ERR_NOMEM,              // 4
    USB_ERR_CANCELLED,          // 5
    USB_ERR_BAD_ADDRESS,        // 6
    USB_ERR_BAD_BUFSIZE,        // 7
    USB_ERR_BAD_FLAG,           // 8
    USB_ERR_NO_CALLBACK,        // 9
    USB_ERR_IN_USE,             // 10
    USB_ERR_NO_ADDR,            // 11
    USB_ERR_NO_PIPE,            // 12
    USB_ERR_ZERO_NFRAMES,       // 13
    USB_ERR_ZERO_MAXP,          // 14
    USB_ERR_SET_ADDR_FAILED,    // 15
    USB_ERR_NO_POWER,           // 16
    USB_ERR_TOO_DEEP,           // 17
    USB_ERR_IOERROR,            // 18
    USB_ERR_NOT_CONFIGURED,     // 19
    USB_ERR_TIMEOUT,            // 20
    USB_ERR_SHORT_XFER,         // 21
    USB_ERR_STALLED,            // 22
    USB_ERR_INTERRUPTED,        // 23
    USB_ERR_DMA_LOAD_FAILED,    // 24
    USB_ERR_BAD_CONTEXT,        // 25
    USB_ERR_NO_ROOT_HUB,        // 26
    USB_ERR_NO_INTR_THREAD,     // 27
    USB_ERR_NOT_LOCKED,         // 28
    USB_ERR_MAX,
}

#[allow(non_camel_case_types)]
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct usb_device_request {
    //usb/usb.h
    pub bmRequestType: u8,
    pub bRequest: u8,
    pub wValue: u16,
    pub wIndex: u16,
    pub wLength: u16,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct usb_port_status {
    pub wPortStatus: u16,
    pub wPortChange: u16,
}


#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct usb_device_descriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub bcdUSB: u16,
    pub bDeviceClass: u8,
    pub bDeviceSubClass: u8,
    pub bDeviceProtocol: u8,
    pub bMaxPacketSize: u8,
    // The fields below are not part of the initial descriptor.
    pub idVendor: u16,
    pub idProduct: u16,
    pub bcdDevice: u16,
    pub iManufacturer: u8,
    pub iProduct: u8,
    pub iSerialNumber: u8,
    pub bNumConfigurations: u8,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct usb_hub_descriptor_min {
    pub bDescLength: u8,
    pub bDescriptorType: u8,
    pub bNbrPorts: u8,
    pub wHubCharacteristics: u16, // Using u16 since uWord is 16-bit
    pub bPwrOn2PwrGood: u8,
    pub bHubContrCurrent: u8,
    pub DeviceRemovable: [u8; 1], // Single-byte array
    pub PortPowerCtrlMask: [u8; 1], // Single-byte array
}


// Constants for USB versions
pub const UD_USB_2_0: u16 = 0x0200;
pub const UD_USB_3_0: u16 = 0x0300;

// Helper functions to check USB version
impl usb_device_descriptor {
    pub fn ud_is_usb2(&self) -> bool {
        (self.bcdUSB >> 8) == 0x02
    }

    pub fn ud_is_usb3(&self) -> bool {
        (self.bcdUSB >> 8) == 0x03
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct usb_config_descriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub wTotalLength: u16,  // Equivalent to uWord in C
    pub bNumInterface: u8,
    pub bConfigurationValue: u8,
    pub iConfiguration: u8,
    pub bmAttributes: u8,
    pub bMaxPower: u8, // Max current in 2 mA units
}

// Constants
pub const USB_UNCONFIG_NO: u8 = 0;

pub const UC_BUS_POWERED: u8 = 0x80;
pub const UC_SELF_POWERED: u8 = 0x40;
pub const UC_REMOTE_WAKEUP: u8 = 0x20;

pub const UC_POWER_FACTOR: u8 = 2;


#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct usb_interface_descriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub bInterfaceNumber: u8,
    pub bAlternateSetting: u8,
    pub bNumEndpoints: u8,
    pub bInterfaceClass: u8,
    pub bInterfaceSubClass: u8,
    pub bInterfaceProtocol: u8,
    pub iInterface: u8,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct usb_endpoint_descriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub bEndpointAddress: u8,
    pub bmAttributes: u8,
    pub wMaxPacketSize: u16, // Equivalent to uWord in C
    pub bInterval: u8,
}

// Macros converted to Rust constants
pub const UE_DIR_IN: u8 = 0x80; // IN-token endpoint, fixed
pub const UE_DIR_OUT: u8 = 0x00; // OUT-token endpoint, fixed
pub const UE_DIR_RX: u8 = 0xfd; // Internal use only
pub const UE_DIR_TX: u8 = 0xfe; // Internal use only
pub const UE_DIR_ANY: u8 = 0xff; // Internal use only

pub const UE_ADDR: u8 = 0x0f;
pub const UE_ADDR_ANY: u8 = 0xff; // Internal use only

pub const UE_XFERTYPE: u8 = 0x03;
pub const UE_CONTROL: u8 = 0x00;
pub const UE_ISOCHRONOUS: u8 = 0x01;
pub const UE_BULK: u8 = 0x02;
pub const UE_INTERRUPT: u8 = 0x03;
pub const UE_BULK_INTR: u8 = 0xfe; // Internal use only
pub const UE_TYPE_ANY: u8 = 0xff; // Internal use only

pub const UE_ISO_TYPE: u8 = 0x0c;
pub const UE_ISO_ASYNC: u8 = 0x04;
pub const UE_ISO_ADAPT: u8 = 0x08;
pub const UE_ISO_SYNC: u8 = 0x0c;

pub const UE_ISO_USAGE: u8 = 0x30;
pub const UE_ISO_USAGE_DATA: u8 = 0x00;
pub const UE_ISO_USAGE_FEEDBACK: u8 = 0x10;
pub const UE_ISO_USAGE_IMPLICT_FB: u8 = 0x20;

pub const UE_ZERO_MPS: u16 = 0xFFFF; // Internal use only


#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct usb_endpoint_ss_comp_descriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub bMaxBurst: u8,
    pub bmAttributes: u8,
    pub wBytesPerInterval: u16,
}
