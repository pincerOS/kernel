// SPDX-License-Identifier: GPL-2.0+
/*
 * Copyright (C) 2012 Oleksandr Tymoshenko <gonzo@freebsd.org>
 * Copyright (C) 2014 Marek Vasut <marex@denx.de>
 * 
 * Modified for Rust by Aaron Lo <aaronlo0929@gmail.com>
 */

use crate::usb::usb_device::udevice;

const DWC2_HC_CHANNEL: usize = 0;
const DWC2_STATUS_BUF_SIZE: usize = 64;
const DWC2_DATA_BUF_SIZE: usize = (64 * 1024); //the 64 can be shrunk

const MAX_DEVICE: usize = 16;
const MAX_ENDPOINT: usize = 16;


struct dwc2_priv {
    align_buf: [u8; DWC2_DATA_BUF_SIZE], //TODO: Make this aligned
    status_buf: [u8; DWC2_STATUS_BUF_SIZE],
    // struct phy phy;
	// struct clk_bulk clks;
    in_data_toggle: [[u8; MAX_ENDPOINT]; MAX_DEVICE],
    out_data_toggle: [[u8; MAX_ENDPOINT]; MAX_DEVICE],
    devnum: u32,
    ext_vbus: bool,
    /*
	 * The hnp/srp capability must be disabled if the platform
	 * does't support hnp/srp. Otherwise the force mode can't work.
	 */
    hnp_srp_disable: bool,
    oc_disable: bool,
    resets: reset_ctl_bulk,
}

//from u-boot/include/reset.h
struct reset_ctl_bulk {
    resets: *mut reset_ctl, // Pointer to an array of reset_ctl
    count: u32,
}

struct reset_ctl {
    dev: *mut udevice,
    id: u64,
    data: u64,
    polarity: u64,
}

static mut dwc2_priv: dwc2_priv = dwc2_priv {
    align_buf: [0; DWC2_DATA_BUF_SIZE],
    status_buf: [0; DWC2_STATUS_BUF_SIZE],
    in_data_toggle: [[0; MAX_ENDPOINT]; MAX_DEVICE],
    out_data_toggle: [[0; MAX_ENDPOINT]; MAX_DEVICE],
    devnum: 0,
    ext_vbus: false,
    hnp_srp_disable: false,
    oc_disable: false,
    resets: reset_ctl_bulk {
        resets: core::ptr::null_mut(),
        count: 0,
    },
};

pub fn dwc2_usb_probe(dev: *mut udevice) {
    let dwc2_priv = (udevice->priv_) as *mut dwc2_priv;
}
