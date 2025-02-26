

use crate::shutdown;

use super::usb_device::*;

pub fn usbd_ctrl_transfer_setup(udev: &mut usb_device) {
    /* check for root HUB */
	if (udev.parent_hub.is_null()) {
        return;
    }

    println!("| usbd_ctrl_transfer_setup: not implemented");
    shutdown();
    return;

    //TODO:
}