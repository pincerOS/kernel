pub mod configuration;
pub mod hcd;
pub mod types;
pub mod usbd;
pub mod device;

use alloc::boxed::Box;
use hcd::dwc::dwc_otg::*;
use usbd::device::*;
use usbd::usbd::*;

pub fn usb_init(base_addr: *mut ()) -> UsbBus {
    let mut bus = UsbBus {
        devices: core::array::from_fn(|_| const { None }),
        interface_class_attach: [None; INTERFACE_CLASS_ATTACH_COUNT],
        dwc_sc: Box::new(dwc_hub::new()),
    };
    // usbd::UsbLoad(&mut bus);
    UsbInitialise(&mut bus, base_addr);

    return bus;
}

pub fn usb_check_for_change(bus: &mut UsbBus) {
    // UsbCheckForChange(bus);
}
