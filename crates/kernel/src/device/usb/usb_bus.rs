
use super::usb_controller::*;

pub fn usb_bus_init(bus: *mut usb_bus) {
}

pub fn usb_bus_lock(bus: *mut usb_bus) {
}

pub fn usb_bus_unlock(bus: *mut usb_bus) {
}


pub struct usb_bus {

    pub methods: Option<*mut usb_bus_methods>,
    pub hw_power_state: u16,
}

