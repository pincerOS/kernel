

pub struct usb_pipe_methods {
    pub open: fn(*mut usb_xfer),
    pub close: fn(*mut usb_xfer),
    pub start: fn(*mut usb_xfer),
    pub stop: fn(*mut usb_xfer),
}