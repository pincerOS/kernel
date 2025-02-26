use crate::shutdown;

use super::usb::usb_error_t;
use super::usbdi::*;
use super::usbreg::*;
use super::usb_bus::*;
use super::usb_hub::*;
use super::usb_core::*;
use super::usb::*;

use alloc::boxed::Box;



pub fn usb_alloc_device(bus: &mut usb_bus, parent_hub: *mut usb_device, depth: u8, port_index: u8, port_no: u8, speed: usb_dev_speed, mode: usb_hc_mode) -> *mut usb_device {
    
    /*
	 * Find an unused device index. In USB Host mode this is the
	 * same as the device address.
	 *
	 * Device index zero is not used and device index 1 should
	 * always be the root hub.
	 */
    let mut device_index = 0;
    for i in USB_ROOT_HUB_ADDR..USB_MAX_DEVICES {
        if bus.devices[i as usize].is_null() {
            device_index = i;
            break;
        }
    }

    if device_index == 0 {
        println!("usb_alloc_device: no free device index");
        return core::ptr::null_mut();
    }

    //care about depth?

    let mut udev = Box::new(usb_device {
        iface: core::ptr::null_mut(),
        ctrl_ep: usb_endpoint {
            endpoint_q: [core::ptr::null_mut(); USB_MAX_EP_STREAMS],
            edesc: core::ptr::null_mut(),
            ecomp: core::ptr::null_mut(),
            methods: core::ptr::null_mut(),
            isoc_next: 0,
            toggle_next: 0,
            is_stalled: 0,
            is_synced: 0,
            unused: 0,
            iface_index: 0,
            refcount_alloc: 0,
            refcount_bw: 0,
            usb_smask: 0,
            usb_cmask: 0,
            usb_uframe: 0,

            ep_mode: usb_ep_mode::USB_EP_MODE_DEFAULT,
        },
        endpoints: [core::ptr::null_mut(); USB_MAX_EP_UNITS],
        cdesc: core::ptr::null_mut(),
        hub: core::ptr::null_mut(),
        ctrl_xfer: [core::ptr::null_mut(); USB_MAX_EP_UNITS],
        ep_curr: core::ptr::null_mut(),
        power: 0,
        

        device_index: device_index as u8,
        parent_hub: parent_hub,
        port_index: port_index,
        port_no: port_no,
        depth: depth,
        bus: bus,
        address: USB_START_ADDR, //default value
        //TODO: udev->power_mode = usbd_filter_power_mode(udev, USB_POWER_MODE_ON);
        speed: speed,
        state: usb_dev_state::USB_STATE_DETACHED,
        ctrl_ep_desc: usb_endpoint_descriptor {
            bLength: size_of::<usb_endpoint_descriptor>() as u8,
            bDescriptorType: UDESC_ENDPOINT,
            bEndpointAddress: USB_CONTROL_ENDPOINT,
            bmAttributes: UE_CONTROL,
            wMaxPacketSize: usetw3(USB_MAX_IPACKET, 0),
            bInterval: 0,
        },
        ctrl_ep_comp_desc: usb_endpoint_ss_comp_descriptor {
            bLength: size_of::<usb_endpoint_ss_comp_descriptor>() as u8,
            bDescriptorType: UDESC_ENDPOINT_SS_COMP,
            bMaxBurst: 0,
            bmAttributes: 0,
            wBytesPerInterval: 0,
        },
        ddesc: usb_device_descriptor {
            bLength: 0,
            bDescriptorType: 0,
            bcdUSB: 0,
            bDeviceClass: 0,
            bDeviceSubClass: 0,
            bDeviceProtocol: 0,
            bMaxPacketSize: USB_MAX_IPACKET,
            idVendor: 0,
            idProduct: 0,
            bcdDevice: 0,
            iManufacturer: 0,
            iProduct: 0,
            iSerialNumber: 0,
            bNumConfigurations: 0,
        },
        flags: usb_device_flags {
            usb_mode: mode,
            self_powered: 0,
            no_strings: 0,
            remote_wakeup: 0,
            uq_bus_powered: 0,
            peer_suspended: 0,
            self_suspended: 0,
        }
    });

    // adev = udev;
	// hub = udev->parent_hub;

	// while (hub) {
	// 	if (hub->speed == USB_SPEED_HIGH) {
	// 		udev->hs_hub_addr = hub->address;
	// 		udev->parent_hs_hub = hub;
	// 		udev->hs_port_no = adev->port_no;
	// 		break;
	// 	}
	// 	adev = hub;
	// 	hub = hub->parent_hub;
	// }

    /* init the default endpoint */
    usb_init_endpoint(&mut udev, 0);

    /* Initialise device */
    if let Some(device_init) = bus.methods.device_init {
        device_init(&mut udev); 
    }
    
    /* set powered device state after device init is complete */
    usb_set_device_state(&mut udev, usb_dev_state::USB_STATE_POWERED);

    if matches!(udev.flags.usb_mode, usb_hc_mode::Host) {

    }

    core::ptr::null_mut()
}

pub fn usb_init_endpoint(udev: &mut usb_device, iface_index: u8) {
    let bus = unsafe { &mut *udev.bus };

    if let Some(endpoint_init) = bus.methods.endpoint_init {
        endpoint_init(udev);
    } else {
        println!("| usb_init_endpoint: endpoint_init not implemented");
        shutdown();
    }

    let mut ep = &mut udev.ctrl_ep;
    let mut edesc = &mut udev.ctrl_ep_desc;
    let mut ecomp = &mut udev.ctrl_ep_comp_desc;

    ep.edesc = edesc;
    ep.ecomp = ecomp;
    ep.iface_index = iface_index;

    /* setup USB stream queues */
	// for (x = 0; x != USB_MAX_EP_STREAMS; x++) {
	// 	TAILQ_INIT(&ep->endpoint_q[x].head);
	// 	ep->endpoint_q[x].command = &usbd_pipe_start;
	// }

    //usbd_set_endpoint_mode
    //do_unlock = usbd_enum_lock(udev);
 
    ep.ep_mode = usb_ep_mode::USB_EP_MODE_DEFAULT;

    if let Some(clear_stall) = bus.methods.clear_stall {
        clear_stall(ep);
    } else {
        println!("| usb_init_endpoint: clear_stall not implemented");
        shutdown();
    }
}

fn usb_set_device_state(udev: &mut usb_device, state: usb_dev_state) {
    udev.state = state;

    let bus = unsafe { &mut *udev.bus };
    if let Some(device_state_change) = bus.methods.device_state_change {
        device_state_change(udev);
    } else {
        println!("| usb_set_device_state: device_state_change not implemented");
        shutdown();
    }
}

pub fn usb_probe_and_attach() -> usb_error_t {
    usb_error_t::USB_ERR_NORMAL_COMPLETION
}

pub struct usb_device {
    pub iface: *mut usb_interface,
    pub ctrl_ep: usb_endpoint,
    pub endpoints: [*mut usb_endpoint; USB_MAX_EP_UNITS],
    
    pub bus: *mut usb_bus,
    pub parent_hub: *mut usb_device,
    pub cdesc: *mut usb_config_descriptor,
    pub hub: *mut usb_hub,
    pub ctrl_xfer: [*mut usb_xfer; USB_MAX_EP_UNITS],

    pub ep_curr: *mut usb_endpoint,


    pub power: u16, /* mA the device uses */
    pub address: u8,  /* device addess */
    pub device_index: u8,   /* device index in "bus->devices" */
    pub port_index: u8, /* parent HUB port index */
    pub port_no: u8,    /* parent HUB port number */
    pub depth: u8,  /* distance from root HUB */

    pub state: usb_dev_state, /* device state */
    pub speed: usb_dev_speed, /* device speed */

    pub flags: usb_device_flags,
    
    pub ctrl_ep_desc: usb_endpoint_descriptor,
    pub ctrl_ep_comp_desc: usb_endpoint_ss_comp_descriptor,
    pub ddesc: usb_device_descriptor,
}


pub struct usb_device_flags {
    usb_mode: usb_hc_mode, // host or device mode
    self_powered: u8,      // set if USB device is self powered
    no_strings: u8,        // set if USB device does not support strings
    remote_wakeup: u8,     // set if remote wakeup is enabled
    uq_bus_powered: u8,    // set if BUS powered quirk is present

    /*
     * NOTE: Although the flags below will reach the same value
     * over time, but the instant values may differ, and
     * consequently the flags cannot be merged into one!
     */
    peer_suspended: u8,    // set if peer is suspended
    self_suspended: u8,    // set if self is suspended
}

enum usb_hc_mode {
    Host,
    Device,
}
