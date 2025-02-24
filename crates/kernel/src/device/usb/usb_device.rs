


pub struct usb_device {
    pub iface: *mut usb_interface,
    pub ctrl_ep: *mut usb_endpoint,
    pub ep: [*mut usb_endpoint; USB_MAX_ENDPOINTS],
    
    pub bus: *mut usb_bus,
    pub parent_hub: *mut usb_hub,
    pub cdesc: *mut usb_config_descriptor,
    pub hub: *mut usb_hub,
    pub ctrl_xfer: [*mut usb_xfer; USB_MAX_ENDPOINTS],

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
