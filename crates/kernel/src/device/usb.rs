
pub mod types;
pub mod usbd;
pub mod configuration;
pub mod hcd;

use usbd::device::*;
use usbd::usbd::*;
use hcd::dwc::dwc_otg::*;

pub fn usb_init(base_addr: *mut()) {
    let mut bus = UsbBus {
        devices: [None; MaximumDevices],
        interface_class_attach: [None; INTERFACE_CLASS_ATTACH_COUNT],
        dwc_sc: dwc_hub::new(),
    };
    // usbd::UsbLoad(&mut bus);
    UsbInitialise(&mut bus, base_addr);
}