
use super::usb_core::*;
use super::usb_device::*;
use super::usb_bus::*;
use super::usb::*;
use super::usbreg::*;
use super::usb_controller::*;

pub struct usb_interface {
    pub idesc: *mut usb_interface_descriptor,
    
}

pub struct usb_xfer_queue {
    //TAILQ_HEAD(, usb_xfer) head;
    pub curr: *mut usb_xfer,
    pub command: fn(*mut usb_xfer),
    pub recurse_1: u8,
    pub recurse_2: u8,
    pub recurse_3: u8,
}

pub struct usb_endpoint {
    /* queue of USB transfers */
    pub endpoint_q: [usb_xfer_queue; USB_MAX_EP_STREAMS as usize],

    pub edesc: *mut usb_endpoint_descriptor,
    pub ecomp: *mut usb_endpoint_ss_comp_descriptor,
    pub methods: *const usb_pipe_methods, /* set by HC driver */

    pub isoc_next: u16,

    pub toggle_next: u8, /* next data toggle value */
    pub is_stalled: u8, /* set if endpoint is stalled */
    pub is_synced: u8, /* set if we a synchronised */
    pub unused: u8,

    pub iface_index: u8, /* not used by "default endpoint" */

    pub refcount_alloc: u8, /* allocation refcount */
    pub refcount_bw: u8,    /* bandwidth refcount */

    /* High-Speed resource allocation (valid if "refcount_bw" > 0) */
    pub usb_smask: u8,  /* USB start mask */
    pub usb_cmask: u8,  /* USB complete mask */
    pub usb_uframe: u8, /* USB microframe */

    /* USB endpoint mode, see USB_EP_MODE_XXX */
    pub ep_mode: u8,
}

pub const USB_EP_REF_MAX: u8 = 0x3F;