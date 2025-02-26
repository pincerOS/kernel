
use super::usb_device::*;
use super::usbdi::*;
use super::usbreg::*;
use super::usb::*;



pub fn usbd_req_get_port_status(
    udev: *mut usb_device,
    // mtx: *mut mtx,
    ps: *mut usb_port_status,
    port: u8,
) -> usb_error_t {

    let mut req = usb_device_request {
        bmRequestType: UT_READ_CLASS_OTHER,
        bRequest: UR_GET_STATUS,
        wValue: 0 as u16,
        wIndex: port as u16,
        wLength: (size_of::<usb_port_status>() as u16),
    };

    // usbd_do_request_flags(udev, mtx, &mut req, ps, 0, core::ptr::null_mut(), 1000)
    usb_error_t::USB_ERR_NORMAL_COMPLETION
}


pub fn usbd_do_request_flags(udev: *mut usb_device, req: *mut usb_device_request) {
    //TODO: locking?

    let hr_func = unsafe { ((*(*udev).bus.methods.unwrap()).roothub_exec)};
    //TODO: USB_BUS_LOCK(udev->bus);

    let (err, data, len) = hr_func(udev, req);
    
}