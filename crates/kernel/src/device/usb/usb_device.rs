use super::usb::usb_error_t;
use super::usbdi::*;
use super::usbreg::*;
use super::usb_bus::*;
use super::usb_hub::*;
use super::usb_core::*;
use super::usb::*;



pub fn usb_alloc_device() -> *mut usb_device {
    // let dev = kmalloc(size_of::<usb_device>(), GFP_KERNEL) as *mut usb_device;
    // if dev.is_null() {
    //     return core::ptr::null_mut();
    // }
    // dev
    core::ptr::null_mut()
}

pub fn usb_probe_and_attach() -> usb_error_t {
    usb_error_t::USB_ERR_NORMAL_COMPLETION
}

pub struct usb_device {
    pub iface: *mut usb_interface,
    pub ctrl_ep: *mut usb_endpoint,
    pub endpoints: [*mut usb_endpoint; USB_MAX_EP_UNITS],
    
    pub bus: usb_bus,
    pub parent_hub: *mut usb_hub,
    pub cdesc: *mut usb_config_descriptor,
    pub hub: *mut usb_hub,
    pub ctrl_xfer: [*mut usb_xfer; USB_MAX_EP_UNITS],

    pub ep_curr: *mut usb_endpoint,


    pub power: u16, /* mA the device uses */
    pub address: u8,  /* device addess */
    pub device_index: u8,   /* device index in "bus->devices" */
    pub port_index: u8, /* parent HUB port index */
    pub port_no: u8,    /* parent HUB port number */


    pub flags: *mut usb_device_flags,
    
    pub ctrl_ep_desc: *mut usb_endpoint_descriptor,
    pub ctrl_ep_comp_desc: *mut usb_endpoint_ss_comp_descriptor,
    pub ddesc: *mut usb_device_descriptor,
}


struct usb_device_flags {
    usb_mode: usb_hc_mode, // host or device mode
    self_powered: u8,      // set if USB device is self powered
    no_strings: u8,        // set if USB device does not support strings
    remote_wakeup: u8,     // set if remote wakeup is enabled
    uq_bus_powered: u8,    // set if BUS powered quirk is present

    /*
     * NOTE: Although the flags below will reach the same value
     * over time, but the instant values may differ, and
     * consequently the flags cannot be merged into one!
     */
    peer_suspended: u8,    // set if peer is suspended
    self_suspended: u8,    // set if self is suspended
}

enum usb_hc_mode {
    Host,
    Device,
}
