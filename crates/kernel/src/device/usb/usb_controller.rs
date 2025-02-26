
use super::usb_bus::*;
use super::usb_device::*;
use super::usbreg::*;
use super::usb::*;
use super::usb_hub::*;
use super::usb_request::*;
use super::usb_core::*;


fn usb_attach() -> u32 {
    //set up the entirity of the usb stack

    let mut bus = usb_bus {
        methods: None,
        hw_power_state: 0,
    };

    usb_bus_init(&mut bus);
    usb_bus_lock(&mut bus);

    //attach the bus
    //usb_bus_attach

    let speed = usb_dev_speed::USB_SPEED_HIGH;

    /* default power_mask value */
    bus.hw_power_state = USB_HW_POWER_CONTROL | USB_HW_POWER_BULK | USB_HW_POWER_INTERRUPT | USB_HW_POWER_ISOC | USB_HW_POWER_NON_ROOT_HUB;
    usb_bus_unlock(&mut bus);

    //TODO: check set_hw_power is not null
    unsafe { ((*bus.methods.unwrap()).set_hw_power) (&mut bus) };

    //allocate the root usb device
    let child = usb_alloc_device();

    if !child.is_null() {
        //usb prob and attach
    } else {
        println!("| usb_attach: usb_alloc_device failed");
    }

    usb_bus_lock(&mut bus);

    //set device softc

    //start power watchdog, ask A


    usb_bus_unlock(&mut bus);
    usb_needs_explore(&mut bus);
    return 0;
}


pub struct usb_pipe_methods {
    pub open: fn(*mut usb_xfer),
    pub close: fn(*mut usb_xfer),
    pub start: fn(*mut usb_xfer),
    pub stop: fn(*mut usb_xfer),
}

pub struct usb_bus_methods {
    pub roothub_exec: fn(*mut usb_device, *mut usb_device_request) -> (usb_error_t, *const core::ffi::c_void, u16),
    pub set_hw_power: fn(*mut usb_bus),
}

pub const USB_HW_POWER_CONTROL: u16 = 0x0001;
pub const USB_HW_POWER_BULK: u16 = 0x0002;
pub const USB_HW_POWER_INTERRUPT: u16 = 0x0004;
pub const USB_HW_POWER_ISOC: u16 = 0x0008;
pub const USB_HW_POWER_NON_ROOT_HUB: u16 = 0x0010;
pub const USB_HW_POWER_SUSPEND: u16 = 0x0020;
pub const USB_HW_POWER_RESUME: u16 = 0x0040;
pub const USB_HW_POWER_SHUTDOWN: u16 = 0x0060;
