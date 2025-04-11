#![allow(nonstandard_style)]

pub mod configuration;
pub mod device;
pub mod hcd;
pub mod types;
pub mod usbd;

pub use device::hid::keyboard;
pub use device::hid::mouse;

use device::net::NetSendPacket;
use device::net::RegisterNetReceiveCallback;

use alloc::boxed::Box;
use hcd::dwc::dwc_otg::*;
use usbd::device::*;
use usbd::usbd::*;

pub fn usb_init(base_addr: *mut ()) -> Box<UsbBus> {
    let mut bus = Box::new(UsbBus {
        devices: core::array::from_fn(|_| const { None }),
        interface_class_attach: [None; INTERFACE_CLASS_ATTACH_COUNT],
        roothub_device_number: 0,
        dwc_sc: Box::new(dwc_hub::new()),
    });
    // usbd::UsbLoad(&mut bus);
    UsbInitialise(&mut bus, base_addr);

    return bus;
}

pub fn usb_check_for_change(_bus: &mut UsbBus) {
    // UsbCheckForChange(bus);
}

pub fn usb_register_net_callback(callback: fn(*mut u8, u32)) {
    RegisterNetReceiveCallback(callback);
}

pub unsafe fn usb_send_packet(buffer: *mut u8, buffer_length: u32) {
    unsafe {
        NetSendPacket(buffer, buffer_length);
    }
}
