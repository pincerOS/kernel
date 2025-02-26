
use super::usb_bus::*;
use super::usb_device::*;
use super::usb::*;

pub fn usb_needs_explore(bus: *mut usb_bus) {

}

fn uhub_explore(udev: *mut usb_device) -> usb_error_t {
    let hub = unsafe { (*udev).hub };
    let sc = unsafe { (*hub).hubsoftc };

    //do_unlock = usbd_enum_lock(udev);

    let mut retval = usb_error_t::USB_ERR_NORMAL_COMPLETION;

    let nports = unsafe { (*hub).nports };

    for x in 0..nports {
        let portno = x + 1;
        let up = unsafe { &(*hub).ports[x as usize] };

        let err = uhub_read_port_status(sc as *mut uhub_softsc, portno);
    }

    retval
}


fn uhub_read_port_status(sc: *mut uhub_softsc, portno: u8) -> usb_error_t {
    usb_error_t::USB_ERR_NORMAL_COMPLETION
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