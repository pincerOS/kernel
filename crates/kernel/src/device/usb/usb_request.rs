
use crate::shutdown;

use super::usb_bus::*;
use super::usb_device::*;
use super::usb_transfer::usbd_ctrl_transfer_setup;
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


pub fn usbd_do_request_flags(udev: &mut usb_device, req: *mut usb_device_request, flags: u16) {
    //TODO: locking
    /*
	 * Serialize access to this function:
	 */
	// do_unlock = usbd_ctrl_lock(udev);

    let mut bus = unsafe { &mut *udev.bus };
    if let Some(hr_func) = bus.methods.roothub_exec {
        usb_bus_lock(&mut bus);
        let (err, data, len) = hr_func(udev, req);
        usb_bus_unlock(&mut bus);

        if err != usb_error_t::USB_ERR_NORMAL_COMPLETION {
            println!("| usbd_do_request_flags: roothub_exec failed");
            shutdown();
        }
    } else {
        println!("| usbd_do_request_flags: roothub_exec not implemented");
        shutdown();
    }

    /*
	 * Setup a new USB transfer or use the existing one, if any:
	 */
    usbd_ctrl_transfer_setup(udev);

    let xfer = &mut udev.ctrl_xfer[0];
    if xfer.is_null() {
        println!("| usbd_do_request_flags: no control xfer");
        shutdown();
    }

    //usb_xfer_lock(xfer);

    //if (flags & USB_DELAY_STATUS_STAGE)
		// xfer->flags.manual_status = 1;
        // else
        //     xfer->flags.manual_status = 0;
    
        // if (flags & USB_SHORT_XFER_OK)
        //     xfer->flags.short_xfer_ok = 1;
        // else
        //     xfer->flags.short_xfer_ok = 0;
    
        // xfer->timeout = timeout;
    
        // start_ticks = ticks;
    
        // max_ticks = USB_MS_TO_TICKS(timeout);
        
    //usbd_copy_in(xfer->frbuffers, 0, req, sizeof(*req));

    
    
}