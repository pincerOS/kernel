


pub fn usbd_req_get_port_status(
    udev: *mut usb_device,
    mtx: *mut mtx,
    ps: *mut usb_port_status,
    port: u8,
) -> UsbErrorT {

    let mut req = UsbDeviceRequest {
        bm_request_type: UT_READ_CLASS_OTHER,
        b_request: UR_GET_STATUS,
        w_value: [0, 0],
        w_index: [port, 0],
        w_length: (size_of::<usb_port_status>() as u16).to_le_bytes(),
    };

    usbd_do_request_flags(udev, mtx, &mut req, ps, 0, core::ptr::null_mut(), 1000)
}


pub fn usbd_do_request_flags(udev: *mut usb_device, req: *mut usb_device_request) {
    //TODO: locking?

    let hr_func = udev.bus.methods.roothub_exec;
    //TODO: USB_BUS_LOCK(udev->bus);

    let (err, data, len) = hr_func(udev, req);
    
}