

fn uhub_explore(udev: *mut usb_device) -> usb_error_t {

    let hub = udev.hub;
    let sc = hub.hubsoftc;

    //do_unlock = usbd_enum_lock(udev);

    let mut retval = USB_ERR_NORMAL_COMPLETION;

    for x in 0..hub.nports {
        let portno = x + 1;
        let up = &hub.ports[x as usize];

        let err = uhub_read_port_status(sc, portno);
    }
}


fn uhub_read_port_status(sc: *mut uhub_softsc, portno: u8) -> usb_error_t {

}   


pub struct usb_hub {
    hubudev: *mut usb_device,
    explore: unsafe fn(*mut usb_device) -> usb_error_t,
    hubsoftc: *mut core::ffi::c_void,

    pub portpower: u16,
    pub nports: u8,
    pub ports: [usb_port; 0], // Variable length array
}

pub struct uhub_softsc {
    sc_st: uhub_current_state,
    sc_hub: usb_hub,
    
    sc_usb_port_errors: u8,
    sc_flag: u8,
}

pub struct uhub_current_state {
    port_change: u16,
    port_status: u16,
}

pub struct usb_port {
    restartcnt: u8,
    device_index: u8,
}