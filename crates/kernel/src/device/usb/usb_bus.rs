



pub struct usb_bus {
    //add a lock
    methods: *mut usb_bus_methods,

}

pub struct usb_bus_methods {
    roothub_exec: fn(*mut usb_device, *mut usb_device_request) -> (usb_error_t, *const core::ffi::c_void, u16),
}