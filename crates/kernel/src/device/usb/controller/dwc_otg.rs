/*-
 * SPDX-License-Identifier: BSD-2-Clause
 *
 * Copyright (c) 2015 Daisuke Aoyama. All rights reserved.
 * Copyright (c) 2012-2015 Hans Petter Selasky. All rights reserved.
 * Copyright (c) 2010-2011 Aleksandr Rybalko. All rights reserved.
 *
 * Modified in 2025 for Rust compatibility and PincerOS by Aaron Lo <aaronlo0929@gmail.com>
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 */
//https://elixir.bootlin.com/freebsd/v14.2/source/sys/dev/usb/controller/dwc_otg.c
/*
 * This file contains the driver for the DesignWare series USB 2.0 OTG
 * Controller.
 */

/*
 * LIMITATION: Drivers must be bound to all OUT endpoints in the
 * active configuration for this driver to work properly. Blocking any
 * OUT endpoint will block all OUT endpoints including the control
 * endpoint. Usually this is not a problem.
 */

/*
 * NOTE: Writing to non-existing registers appears to cause an
 * internal reset.
 */


use crate::device::gic;
use crate::context::Context;
// mod dwc_otgreg;
use super::dwc_otgreg::*;
use crate::device::usb::*;
// use crate::device::usbreg::*;
use crate::shutdown;
use crate::sync::{SpinLockInner, UnsafeInit};
use crate::device::system_timer::{self, SYSTEM_TIMER};
use core::default;
use core::ptr;
use core::ptr::read;
use alloc::boxed::Box;

use super::super::usb_bus::*;
use super::super::usb_controller::*;
use super::super::usb_core::*;
use super::super::usb_device::*;
use super::super::usb_request::*;
use super::super::usb_transfer::*;
use super::super::usbdi::*;
use super::super::usbreg::*;
use super::super::usb::*;

pub static mut dwc_otg_driver: DWC_OTG = DWC_OTG { base_addr: 0 };
pub static mut dwc_otg_sc: *mut dwc_otg_softc = core::ptr::null_mut();

pub fn initialize_dwc_otg_sc() {
    let softc = Box::new(dwc_otg_softc::new());
    unsafe {
        dwc_otg_sc = Box::into_raw(softc); // Convert Box to raw pointer
    }
}

pub fn register_dwc_otg_interrupt_handler() {
    gic::GIC.get().register_isr(105, dwc_otg_interrupt_handler);
}

fn dwc_otg_interrupt_handler(ctx: &mut Context) {
    let retval = dwc_otg_filter_interrupt(&ctx);
    println!("| retval: 0x{:08x}", retval);
    if retval == FILTER_SCHEDULE_THREAD {
        dwc_otg_interrupt(ctx);
    }
}

fn dwc_otg_clocks_on(sc: &mut dwc_otg_softc)
{
	if sc.sc_flags.clocks_off != 0 && sc.sc_flags.port_powered != 0 {
		/* TODO - platform specific */
		sc.sc_flags.clocks_off = 0;
	}
}

fn dwc_otg_clocks_off(sc: &mut dwc_otg_softc)
{
	if sc.sc_flags.clocks_off == 0 {
		/* TODO - platform specific */
		sc.sc_flags.clocks_off = 1;
	}
}

fn dwc_otg_pull_up(sc: &mut dwc_otg_softc)
{
	/* pullup D+, if possible */
	if (sc.sc_flags.d_pulled_up == 0 && sc.sc_flags.port_powered != 0) {
		sc.sc_flags.d_pulled_up = 1;

		let mut temp = read_volatile(DCTL);
		temp &= !DCTL_SFTDISCON;
		write_volatile(DCTL, temp);
	}
}

fn dwc_otg_pull_down(sc: &mut dwc_otg_softc)
{
	/* pulldown D+, if possible */
	if sc.sc_flags.d_pulled_up != 0 {
		sc.sc_flags.d_pulled_up = 0;

		let mut temp = read_volatile(DCTL);
		temp |= DCTL_SFTDISCON;
		write_volatile(DCTL, temp);
	}
}

pub const DWC_OTG_INTR_ENDPT: u8 = 1;

pub fn dwc_otg_roothub_exec(sc: &mut dwc_otg_softc, req: usb_device_request) -> (usb_error_t, *const core::ffi::c_void, u16) {

    //USB_BUS_LOCK_ASSERT(&sc->sc_bus, MA_OWNED);

    /* buffer reset */
    let mut ptr = &mut sc.sc_hub_temp as *const _ as *const core::ffi::c_void;
    let mut len = 0;
	let mut err = usb_error_t::USB_ERR_NORMAL_COMPLETION;

    // value = UGETW(req->wValue); //TODO: What is the point of UGETW
	// index = UGETW(req->wIndex);

    let mut value = req.wValue;
    let index = req.wIndex;

    /* demultiplex the control request */
    match req.bmRequestType {
        UT_READ_DEVICE => {
            match req.bRequest {
                UR_GET_DESCRIPTOR => {
                    let match_value = (value >> 8) as u8;
                    match match_value {
                        UDESC_DEVICE => {
                            if (value & 0xff) != 0 {
                                err = usb_error_t::USB_ERR_STALLED;
                            } else {
                                len = core::mem::size_of::<usb_device_descriptor>() as u16;
                                ptr = &dwc_otg_devd as *const _ as *const core::ffi::c_void;
                            }
                        }
                        UDESC_CONFIG => {
                            if (value & 0xff) != 0 {
                                err = usb_error_t::USB_ERR_STALLED;
                            } else {
                                len = core::mem::size_of::<usb_config_descriptor>() as u16;
                                ptr = &dwc_otg_confd as *const _ as *const core::ffi::c_void;
                            }
                        }
                        UDESC_STRING => {
                            println!("| UR_GET_DESCRIPTOR: UDESC_STRING Not implemented");
                            shutdown();
                        }
                        _ => {}
                    }
                } 
                UR_GET_CONFIG => {
                    len = 1;
                    unsafe { usetw_lower(&mut sc.sc_hub_temp.wValue, sc.sc_conf) };
                }
                UR_GET_STATUS => {
                    len = 2;
                    unsafe { usetw(&mut sc.sc_hub_temp.wValue, UDS_SELF_POWERED) };
                }
                _ => {
                    err = usb_error_t::USB_ERR_STALLED;
                }
            }   
        }
        UT_WRITE_DEVICE => {
            match req.bRequest {
                UR_SET_ADDRESS => {
                    if (value & 0xFF00) != 0 {
                        err = usb_error_t::USB_ERR_STALLED;
                    } else {
                        sc.sc_rt_addr = value as u8;
                    }
                }
                UR_SET_CONFIG => {
                    if value >= 2 {
                        err = usb_error_t::USB_ERR_STALLED;
                    } else {
                        sc.sc_conf = value as u8;
                    }
                }
                UR_CLEAR_FEATURE => {
                    /* nop */
                }
                UR_SET_DESCRIPTOR => {
                    /* nop */
                }
                UR_SET_FEATURE => {
                    err = usb_error_t::USB_ERR_STALLED;
                }
                _ => {
                    err = usb_error_t::USB_ERR_STALLED;
                }
            }
        }
        UT_WRITE_ENDPOINT => {
            match req.bRequest {
                UR_CLEAR_FEATURE => {
                    match req.wValue {
                        UF_ENDPOINT_HALT => {
                            /* noop */
                        }
                        UF_DEVICE_REMOTE_WAKEUP => {
                            /* noop */
                        }
                        _ => {
                            err = usb_error_t::USB_ERR_STALLED;
                        }
                    }
                }
                UR_SET_FEATURE => {
                    match req.wValue {
                        UF_ENDPOINT_HALT => {
                            /* nop */
                        }
                        UF_DEVICE_REMOTE_WAKEUP => {
                            /* nop */
                        }
                        _ => {
                            err = usb_error_t::USB_ERR_STALLED;
                        }
                    }
                }
                UR_SYNCH_FRAME => {
                    /* nop */
                }
                _ => {
                    err = usb_error_t::USB_ERR_STALLED;
                }
            }
        }
        UT_READ_ENDPOINT => {
            match req.bRequest {
                UR_GET_STATUS => {
                    len = 2;
                    unsafe { usetw(&mut sc.sc_hub_temp.wValue, 0) };
                }
                _ => {
                    err = usb_error_t::USB_ERR_STALLED;
                }
            }
        }
        UT_WRITE_INTERFACE => {
            match req.bRequest {
                UR_SET_INTERFACE => {
                    /* nop */
                }
                UR_CLEAR_FEATURE => {
                    /* nop */
                }
                UR_SET_FEATURE => {
                    err = usb_error_t::USB_ERR_STALLED;
                }
                _ => {
                    err = usb_error_t::USB_ERR_STALLED;
                }
            }
        }
        UT_READ_INTERFACE => {
            match req.bRequest {
                UR_GET_INTERFACE => {
                    len = 1;
                    // sc->sc_hub_temp.wValue[0] = 0;
                    unsafe { usetw_lower(&mut sc.sc_hub_temp.wValue, 0) };
                }
                UR_GET_STATUS => {
                    len = 2;
                    sc.sc_hub_temp.wValue = 0;
                }
                _ => {
                    err = usb_error_t::USB_ERR_STALLED;
                }
            }
        }
        UT_WRITE_CLASS_INTERFACE => {
            /* nop */
        }
        UT_WRITE_VENDOR_INTERFACE => {
            /* nop */
        }
        UT_READ_CLASS_INTERFACE => {
            /* nop */
        }
        UT_READ_VENDOR_INTERFACE => {
            /* nop */
        }
        UT_WRITE_CLASS_DEVICE => {
            /* nop */
        }
        UT_WRITE_CLASS_OTHER => {
            match req.bRequest {
                UR_CLEAR_FEATURE => {
                    if index != 1 {
                        err = usb_error_t::USB_ERR_STALLED;
                    } else {
                        println!("| UR_CLEAR_FEATURE: on port {}", index);
                        match value {
                            UHF_PORT_SUSPEND => {
                                println!("| UR_CLEAR_FEATURE: UHF_PORT_SUSPEND not implementd");
                                shutdown();
                            }
                            UHF_PORT_ENABLE => {
                                if sc.sc_flags.status_device_mode == 0 {
                                    write_volatile(HPRT,
                                        sc.sc_hprt_val | HPRT_PRTENA);
                                }
                                sc.sc_flags.port_enabled = 0;
                            }
                            UHF_C_PORT_RESET => {
                                sc.sc_flags.change_reset = 0;
                            }
                            UHF_C_PORT_ENABLE => {
                                sc.sc_flags.change_enabled = 0;
                            }
                            UHF_C_PORT_OVER_CURRENT => {
                                sc.sc_flags.change_over_current = 0;
                            }
                            UHF_PORT_TEST => {
                                /* nop */
                            }
                            UHF_PORT_INDICATOR => {
                                /* nops */
                            }
                            UHF_PORT_POWER => {
                                sc.sc_flags.port_powered = 0;
                                if (sc.sc_mode == DWC_MODE_HOST || sc.sc_mode == DWC_MODE_OTG) {
                                    sc.sc_hprt_val = 0;
                                    write_volatile(HPRT, HPRT_PRTENA);
                                }
                                dwc_otg_pull_down(sc);
                                dwc_otg_clocks_off(sc);
                            }
                            UHF_C_PORT_CONNECTION => {
                                /* clear connect change flag */
		                        sc.sc_flags.change_connect = 0;
                            }
                            UHF_C_PORT_SUSPEND => {
                                sc.sc_flags.change_suspend = 0;
                            }
                            _ => {
                                err = usb_error_t::USB_ERR_IOERROR;
                            }
                        }
                    }
                }
                UR_SET_FEATURE => {
                    if index != 1 {
                        err = usb_error_t::USB_ERR_STALLED;
                    } else {
                        match value {
                            UHF_PORT_ENABLE => {
                                /* noop */
                            }
                            UHF_PORT_SUSPEND => {
                                if sc.sc_flags.status_device_mode == 0 {
                                    /* set suspend BIT */
                                    sc.sc_hprt_val |= HPRT_PRTSUSP;
                                    write_volatile( HPRT, sc.sc_hprt_val);
                                    /* generate HUB suspend event */
                                    dwc_otg_suspend_irq(sc);
                                }
                            }
                            UHF_PORT_RESET => {
                                if sc.sc_flags.status_device_mode == 0 {
                                    println!("| Dwc otg PORT RESET\n");
    
                                    /* enable PORT reset */
                                    write_volatile(HPRT, sc.sc_hprt_val | HPRT_PRTRST);
    
                                    /* Wait 62.5ms for reset to complete */
                                    // usb_pause_mtx(&sc->sc_bus.bus_mtx, hz / 16);
                                    usb_pause_mtx(1000/16);
    
                                    write_volatile(HPRT, sc.sc_hprt_val);
    
                                    /* Wait 62.5ms for reset to complete */
                                    usb_pause_mtx(1000/16);
    
                                    /* reset FIFOs */
                                    // dwc_otg_init_fifo(sc, DWC_MODE_HOST);
                                    dwc_otg_init_fifo(sc);
    
                                    sc.sc_flags.change_reset = 1;
                                } else {
                                    err = usb_error_t::USB_ERR_IOERROR;
                                }
                            }
                            UHF_PORT_TEST => {
                                /* nop */
                            }
                            UHF_PORT_INDICATOR => {
                                /* nops */
                            }
                            UHF_PORT_POWER => {
                                sc.sc_flags.port_powered = 1;
                                if (sc.sc_mode == DWC_MODE_HOST || sc.sc_mode == DWC_MODE_OTG) {
                                    sc.sc_hprt_val |= HPRT_PRTPWR;
                                    write_volatile(HPRT, sc.sc_hprt_val);
                                }
                                if (sc.sc_mode == DWC_MODE_DEVICE || sc.sc_mode == DWC_MODE_OTG) {
                                    /* pull up D+, if any */
                                    dwc_otg_pull_up(sc);
                                }
                            }
                            _ => {
                                err = usb_error_t::USB_ERR_IOERROR;
                            }
                        }
                    }
                }
                UR_CLEAR_TT_BUFFER => {
                    /* nop */
                }
                UR_RESET_TT => {
                    /* nop */
                }
                UR_STOP_TT => {
                    /* nop */
                }  
                _ => {
                    err = usb_error_t::USB_ERR_STALLED;
                }
            }
        }
        UT_READ_CLASS_OTHER => {
            match req.bRequest {
                UR_GET_TT_STATE => {
                    len = 2;
                    unsafe { usetw(&mut sc.sc_hub_temp.wValue, 0) };
                }
                UR_GET_STATUS => {
                    println!("| UR_GET_PORT_STATUS:");

                    if index != 1 {
                        err = usb_error_t::USB_ERR_STALLED;
                    } else {
                        if sc.sc_flags.status_vbus != 0 {
                            dwc_otg_clocks_on(sc);
                        } else {
                            dwc_otg_clocks_off(sc);
                        }

                        /* Select Device Side Mode */
                        if sc.sc_flags.status_device_mode != 0 {
                            value = UPS_PORT_MODE_DEVICE;
                            dwc_otg_timer_stop(sc);
                        } else {
                            value = 0;
                            dwc_otg_timer_start(sc);
                        }
                    
                        if sc.sc_flags.status_high_speed != 0 {
                            value |= UPS_HIGH_SPEED;
                        }
                        else if sc.sc_flags.status_low_speed != 0 {
                            value |= UPS_LOW_SPEED;
                        }

                        if sc.sc_flags.port_powered != 0 {
                            value |= UPS_PORT_POWER;
                        }

                        if sc.sc_flags.port_enabled != 0 {
                            value |= UPS_PORT_ENABLED;
                        }

                        if sc.sc_flags.port_over_current != 0 {
                            value |= UPS_OVERCURRENT_INDICATOR;
                        }

                        if sc.sc_flags.status_vbus != 0 && sc.sc_flags.status_bus_reset != 0 {
                            value |= UPS_CURRENT_CONNECT_STATUS;
                        }

                        if sc.sc_flags.status_suspend != 0 {
                            value |= UPS_SUSPEND;
                        }

                        unsafe { usetw(&mut sc.sc_hub_temp.ps.wPortStatus, value) };
                        value = 0;

                        if sc.sc_flags.change_enabled != 0 {
                            value |= UPS_C_PORT_ENABLED;
                        }
                        if sc.sc_flags.change_connect != 0 {
                            value |= UPS_C_CONNECT_STATUS;
                        }
                        if sc.sc_flags.change_suspend != 0 {
                            value |= UPS_C_SUSPEND;
                        }
                        if sc.sc_flags.change_reset != 0 {
                            value |= UPS_C_PORT_RESET;
                        }
                        if sc.sc_flags.change_over_current != 0 {
                            value |= UPS_C_OVERCURRENT_INDICATOR;
                        }

                        unsafe { usetw(&mut sc.sc_hub_temp.ps.wPortChange, value) };
                        // len = sizeof(sc->sc_hub_temp.ps);
                        len = core::mem::size_of::<usb_port_status>() as u16;
                    }
                }
                _ => {
                    err = usb_error_t::USB_ERR_STALLED;
                }
            }
        }
        UT_READ_CLASS_DEVICE => {
            match req.bRequest {
                UR_GET_DESCRIPTOR => {
                    if (value & 0xFF) != 0 {
                        err = usb_error_t::USB_ERR_STALLED;
                    } else {
                        len = core::mem::size_of::<usb_hub_descriptor_min>() as u16;
                        ptr = &dwc_otg_hubd as *const _ as *const core::ffi::c_void;
                    }
                }
                UR_GET_STATUS => {
                    len = 2;
                    unsafe { usetw(&mut sc.sc_hub_temp.wValue, 0) };
                }
                _ => {
                    err = usb_error_t::USB_ERR_STALLED;
                }
            }
        }
        _ => {
            println!("| dwc_otg_roothub_exec: default error");
            err = usb_error_t::USB_ERR_STALLED;
        }
    }

    return (err, ptr, len);
}

fn dwc_otg_timer_start(sc: &mut dwc_otg_softc)
{
	if sc.sc_timer_active != 0 {
		return;
    }

	sc.sc_timer_active = 1;

	/* restart timer */
	// usb_callout_reset(&sc->sc_timer,
	//     hz / (1000 / DWC_OTG_HOST_TIMER_RATE),
	//     &dwc_otg_timer, sc);
    usb_callout_reset();
}

fn dwc_otg_timer_stop(sc: &mut dwc_otg_softc)
{
	if sc.sc_timer_active == 0 {
		return;
    }

	sc.sc_timer_active = 0;

	/* stop timer */
	// usb_callout_stop(sc);
    usb_callout_stop();
}

//TODO: This should run at high priority, even overriding disabled interrupts
fn dwc_otg_filter_interrupt(_ctx: &Context) -> u32 {
    let mut retval = FILTER_HANDLED;
    let mut sc = unsafe { &mut *dwc_otg_sc };
    //USB_BUS_SPIN_LOCK(&sc->sc_bus);

    /* read and clear interrupt status */
    let status = read_volatile(GINTSTS);
    /* clear interrupts we are handling here */
    write_volatile(GINTSTS, status & !DWC_OTG_MSK_GINT_THREAD_IRQ);

    /* check for USB state change interrupts */
    if (status & DWC_OTG_MSK_GINT_THREAD_IRQ) != 0 {
        retval = FILTER_SCHEDULE_THREAD;
    }

    /* clear FIFO empty interrupts */
    if status & sc.sc_irq_mask & (GINTSTS_PTXFEMP | GINTSTS_NPTXFEMP) != 0 {
        sc.sc_irq_mask &= !(GINTSTS_PTXFEMP | GINTSTS_NPTXFEMP);
        write_volatile(GINTMSK, sc.sc_irq_mask);
    }
    /* clear all IN endpoint interrupts */
    if status & GINTSTS_IEPINT != 0 {
        for x in 0..sc.sc_dev_in_ep_max {
            let temp = read_volatile(DIEPINT(x as u32) as usize);
            if temp != 0 {
                write_volatile(DIEPINT(x as u32) as usize, temp);
            }
        }
    }
    /* poll FIFOs, if any */
    dwc_otg_interrupt_poll_locked(sc);

    if sc.sc_xfer_complete != 0 {
        retval = FILTER_SCHEDULE_THREAD;
    }

    // USB_BUS_SPIN_UNLOCK(&sc->sc_bus);
    return retval;
}

fn dwc_otg_suspend_irq(sc: &mut dwc_otg_softc)
{
	if sc.sc_flags.status_suspend == 0 {
		/* update status bits */
		sc.sc_flags.status_suspend = 1;
		sc.sc_flags.change_suspend = 1;

		if (sc.sc_flags.status_device_mode) != 0 {
			/*
			 * Disable suspend interrupt and enable resume
			 * interrupt:
			 */
			sc.sc_irq_mask &= !GINTMSK_USBSUSPMSK;
			sc.sc_irq_mask |= GINTMSK_WKUPINTMSK;
			write_volatile(GINTMSK, sc.sc_irq_mask);
		}

		/* complete root HUB interrupt endpoint */
		dwc_otg_root_intr(sc);
	}
}

fn dwc_otg_resume_irq(sc: &mut dwc_otg_softc)
{
	if (sc.sc_flags.status_suspend != 0) {
		/* update status bits */
		sc.sc_flags.status_suspend = 0;
		sc.sc_flags.change_suspend = 1;

		if (sc.sc_flags.status_device_mode != 0) {
			/*
			 * Disable resume interrupt and enable suspend
			 * interrupt:
			 */
			sc.sc_irq_mask &= !GINTMSK_WKUPINTMSK;
			sc.sc_irq_mask |= GINTMSK_USBSUSPMSK;
			write_volatile(GINTMSK, sc.sc_irq_mask);
		}

		/* complete root HUB interrupt endpoint */
		dwc_otg_root_intr(sc);
	}
}

//TODO: Scary message: https://elixir.bootlin.com/freebsd/v14.2/source/sys/dev/usb/controller/dwc_otg.c#L465
fn dwc_otg_update_host_frame_interval(sc: &mut dwc_otg_softc) {

	/* setup HOST frame interval register, based on existing value */
	let mut temp = read_volatile(HFIR) & HFIR_FRINT_MASK;
	if (temp >= 10000) {
		temp /= 1000;
    }
	else {
		temp /= 125;
    }

	/* figure out nearest X-tal value */
	if (temp >= 54) {
		temp = 60;	/* MHz */
    }
	else if (temp >= 39) {
		temp = 48;	/* MHz */
    }
	else {
		temp = 30;	/* MHz */
    }

	if (sc.sc_flags.status_high_speed) != 0 {
		temp *= 125;
    }
	else {
		temp *= 1000;
    }

    println!("| HFIR=0x{:08x}", temp);

	write_volatile(HFIR, temp);
}


//Runs in regular / realtime thread context
fn dwc_otg_interrupt(_ctx: &mut Context) {
    let mut sc = unsafe { &mut *dwc_otg_sc };

    // USB_BUS_LOCK(&sc->sc_bus);
	// USB_BUS_SPIN_LOCK(&sc->sc_bus); Why double lock?

    /* read and clear interrupt status */
    let status = read_volatile(GINTSTS);
    /* clear interrupts we are handling here */
	write_volatile(GINTSTS, status & DWC_OTG_MSK_GINT_THREAD_IRQ);
    println!("| GINTSTS=0x{:08x} HAINT=0x{:08x} HFNUM=0x{:08x}", status, read_volatile(HAINT), read_volatile(HFNUM));

	if (status & GINTSTS_USBRST) != 0 {
		/* set correct state */
		sc.sc_flags.status_device_mode = 1;
		sc.sc_flags.status_bus_reset = 0;
		sc.sc_flags.status_suspend = 0;
		sc.sc_flags.change_suspend = 0;
		sc.sc_flags.change_connect = 1;

		/* Disable SOF interrupt */
		sc.sc_irq_mask &= !GINTMSK_SOFMSK;
		write_volatile( GINTMSK, sc.sc_irq_mask);

		/* complete root HUB interrupt endpoint */
		dwc_otg_root_intr(sc);
	}

    /* check for any bus state change interrupts */
	if (status & GINTSTS_ENUMDONE) != 0 {
        println!("| end of reset");

		/* set correct state */
		sc.sc_flags.status_device_mode = 1;
		sc.sc_flags.status_bus_reset = 1;
		sc.sc_flags.status_suspend = 0;
		sc.sc_flags.change_suspend = 0;
		sc.sc_flags.change_connect = 1;
		sc.sc_flags.status_low_speed = 0;
		sc.sc_flags.port_enabled = 1;

		/* reset FIFOs */
        println!("| dwc_otg_interrupt: this should not run...");
        shutdown();
		// (void) dwc_otg_init_fifo(sc, DWC_MODE_DEVICE);

		/* reset function address */
		// dwc_otg_set_address(sc, 0);

		// /* figure out enumeration speed */
		// temp = DWC_OTG_READ_4(sc, DOTG_DSTS);
		// if (DSTS_ENUMSPD_GET(temp) == DSTS_ENUMSPD_HI)
		// 	sc->sc_flags.status_high_speed = 1;
		// else
		// 	sc->sc_flags.status_high_speed = 0;

		// /*
		//  * Disable resume and SOF interrupt, and enable
		//  * suspend and RX frame interrupt:
		//  */
		// sc->sc_irq_mask &= ~(GINTMSK_WKUPINTMSK | GINTMSK_SOFMSK);
		// sc->sc_irq_mask |= GINTMSK_USBSUSPMSK;
		// DWC_OTG_WRITE_4(sc, DOTG_GINTMSK, sc->sc_irq_mask);

		/* complete root HUB interrupt endpoint */
		// dwc_otg_root_intr(sc);
	}

    if (status & GINTSTS_PRTINT) != 0 {

		let hprt = read_volatile(HPRT);
        println!("HPRT = 0x{:08x}", hprt);
		/* clear change bits */
        write_volatile(HPRT, (hprt & (HPRT_PRTPWR | HPRT_PRTENCHNG | HPRT_PRTCONNDET | HPRT_PRTOVRCURRCHNG)) | sc.sc_hprt_val);

        println!("| GINTSTS=0x{:08x}, HPRT=0x{:08x}", status, hprt);

		sc.sc_flags.status_device_mode = 0;

		if (hprt & HPRT_PRTCONNSTS) != 0 {
			sc.sc_flags.status_bus_reset = 1;
        }
		else {
			sc.sc_flags.status_bus_reset = 0;
        }

		if ((hprt & HPRT_PRTENCHNG) != 0) && ((hprt & HPRT_PRTENA) == 0) {
			sc.sc_flags.change_enabled = 1;
        }

		if (hprt & HPRT_PRTENA) != 0 {
			sc.sc_flags.port_enabled = 1;
        } else {
			sc.sc_flags.port_enabled = 0;
        }

		if (hprt & HPRT_PRTOVRCURRCHNG) != 0 {
			sc.sc_flags.change_over_current = 1;
        }

		if (hprt & HPRT_PRTOVRCURRACT) != 0 {
			sc.sc_flags.port_over_current = 1;
        }
		else {
			sc.sc_flags.port_over_current = 0;
        }

		if (hprt & HPRT_PRTPWR) != 0 {
			sc.sc_flags.port_powered = 1;
        }
		else {
			sc.sc_flags.port_powered = 0;
        }

		if (((hprt & HPRT_PRTSPD_MASK) >> HPRT_PRTSPD_SHIFT) == HPRT_PRTSPD_LOW) {
			sc.sc_flags.status_low_speed = 1;
        }
		else {
			sc.sc_flags.status_low_speed = 0;
        }

		if (((hprt & HPRT_PRTSPD_MASK) >> HPRT_PRTSPD_SHIFT) == HPRT_PRTSPD_HIGH) {
            sc.sc_flags.status_high_speed = 1;
        }
		else {
			sc.sc_flags.status_high_speed = 0;
        }

		if (hprt & HPRT_PRTCONNDET) != 0 {
			sc.sc_flags.change_connect = 1;
        }

		if (hprt & HPRT_PRTSUSP) != 0 {
			dwc_otg_suspend_irq(sc);
        }
		else {
			dwc_otg_resume_irq(sc);
        }

		/* complete root HUB interrupt endpoint */
		dwc_otg_root_intr(sc);

		/* update host frame interval */
		dwc_otg_update_host_frame_interval(sc);
	}

    /*
	 * If resume and suspend is set at the same time we interpret
	 * that like RESUME. Resume is set when there is at least 3
	 * milliseconds of inactivity on the USB BUS.
	 */
	if (status & GINTSTS_WKUPINT) != 0 {
        println!("| resume interrupt");

		dwc_otg_resume_irq(sc);

	} else if (status & GINTSTS_USBSUSP) != 0 {
        println!("| suspend interrupt");
		dwc_otg_suspend_irq(sc);
	}
	/* check VBUS */
	if status & (GINTSTS_USBSUSP | GINTSTS_USBRST | GINTMSK_OTGINTMSK | GINTSTS_SESSREQINT) != 0 {
		let temp = read_volatile(GOTGCTL);

        println!("| GOTGCTL=0x{:08x}", temp);

        let mut is_on = 0;
        if temp & (GOTGCTL_ASESVLD | GOTGCTL_BSESVLD) != 0 {
            is_on = 1;
        }
		dwc_otg_vbus_interrupt(sc,is_on);
	}

	if sc.sc_xfer_complete != 0 {
		sc.sc_xfer_complete = 0;

		/* complete FIFOs, if any */
		dwc_otg_interrupt_complete_locked(sc);
	}
	// USB_BUS_SPIN_UNLOCK(&sc->sc_bus);
	// USB_BUS_UNLOCK(&sc->sc_bus);
}

//TODO: TODO: Implement
fn dwc_otg_update_host_transfer_schedule_locked(sc: &mut dwc_otg_softc) -> u8 {
    0
}

//TODO: TODO: Implement
fn dwc_otg_xfer_do_fifo(sc: &mut dwc_otg_softc) {

}

fn dwc_otg_common_rx_ack(sc: &mut dwc_otg_softc) {
    println!("| RX status cleared");

    /* enable RX FIFO level interrupt */
	sc.sc_irq_mask |= GINTMSK_RXFLVLMSK;
	write_volatile(GINTMSK, sc.sc_irq_mask);

	if sc.sc_current_rx_bytes != 0 {
		/* need to dump remaining data */

        println!("| dwc_otg_common_rx_ack: not implemented");
        //TODO: TODO:
		// bus_space_read_region_4(sc->sc_io_tag, sc->sc_io_hdl,
		//     sc->sc_current_rx_fifo, sc->sc_bounce_buffer,
		//     sc->sc_current_rx_bytes / 4);
		/* clear number of active bytes to receive */
		sc.sc_current_rx_bytes = 0;
	}
	/* clear cached status */
	sc.sc_last_rx_status = 0;
}

//TODO: TODO: Implement
fn dwc_otg_interrupt_complete_locked(sc: &mut dwc_otg_softc) {

}

fn dwc_otg_interrupt_poll_locked(sc: &mut dwc_otg_softc) {

    let mut got_rx_status = 0;
    if sc.sc_flags.status_device_mode == 0 {
        dwc_otg_update_host_transfer_schedule_locked(sc);
    }


    for _ in 0..16 {
        /* get all host channel interrupts */
        let mut haint = read_volatile(HAINT);

        while haint != 0 {
            let x = haint.trailing_zeros() as usize; // Find first set bit (0-based index)
            
            if x >= sc.sc_host_ch_max as usize {
                break;
            }
    
            let mut temp = read_volatile(HCINT(x));
            write_volatile( HCINT(x), temp);
            temp &= !HCINT_SOFTWARE_ONLY; // Mask out software-only interrupt flags
    
            sc.sc_chan_state[x].hcint |= temp;
            haint &= !(1 << x);
        }

        if sc.sc_last_rx_status == 0 {
            let temp = read_volatile(GINTSTS);
            if temp & GINTSTS_RXFLVL != 0 {
                sc.sc_last_rx_status = read_volatile(GRXSTSP);
            }

            if sc.sc_last_rx_status != 0 {
                let temp = sc.sc_last_rx_status & GRXSTSRD_PKTSTS_MASK;

                /* non-data messages we simply skip */
                if temp != GRXSTSRD_STP_DATA && temp != GRXSTSRD_STP_COMPLETE && temp != GRXSTSRD_OUT_DATA {
                    /* check for halted channel */
                    if temp == GRXSTSRH_HALTED {
                        let chnum = GRXSTSRD_CHNUM_GET(sc.sc_last_rx_status);
                        sc.sc_chan_state[chnum as usize].wait_halted = 0;
                        println!("| Channel {} halt completed", chnum);
                    }
                    /* store bytes and FIFO offset */
                    sc.sc_current_rx_bytes = 0;
                    sc.sc_current_rx_fifo = 0;

                    /* acknowledge status */
                    dwc_otg_common_rx_ack(sc);
                    continue;
                }

                let temp = GRXSTSRD_BCNT_GET(sc.sc_last_rx_status);
                let ep_no = GRXSTSRD_CHNUM_GET(sc.sc_last_rx_status);
    
                /* store bytes and FIFO offset */
                sc.sc_current_rx_bytes = (temp as u16 + 3) & !3;
                sc.sc_current_rx_fifo = DOTG_DFIFO(ep_no) as u16;
                println!("| Reading {} bytes from ep {}", temp, ep_no);

                /* check if we should dump the data */
                if (sc.sc_active_rx_ep & (1 << ep_no)) == 0 {
                    dwc_otg_common_rx_ack(sc);
                    continue;
                }

                got_rx_status = 1;
                // DPRINTFN(5, "RX status = 0x%08x: ch=%d pid=%d bytes=%d sts=%d\n",
			    // sc->sc_last_rx_status, ep_no,
			    // (sc->sc_last_rx_status >> 15) & 3,
			    // GRXSTSRD_BCNT_GET(sc->sc_last_rx_status),
			    // (sc->sc_last_rx_status >> 17) & 15);
                println!("| RX status = 0x{:08x}: ch={} pid={} bytes={} sts={}", sc.sc_last_rx_status, ep_no, (sc.sc_last_rx_status >> 15) & 3, GRXSTSRD_BCNT_GET(sc.sc_last_rx_status), (sc.sc_last_rx_status >> 17) & 15);
            } else {
                got_rx_status = 0;
            }
        } else {
            let ep_no = GRXSTSRD_CHNUM_GET(sc.sc_last_rx_status);
            /* check if we should dump the data */
            if (sc.sc_active_rx_ep & (1 << ep_no)) == 0 {
                dwc_otg_common_rx_ack(sc);
                continue;
            }

            got_rx_status = 1;
        }
        /* execute FIFOs */
        //TODO: TODO: Implement
        // TAILQ_FOREACH(xfer, &sc->sc_bus.intr_q.head, wait_entry)
		// dwc_otg_xfer_do_fifo(sc, xfer);

        if got_rx_status == 1 {
            /* check if data was consumed */
            if sc.sc_last_rx_status == 0 {
                continue;
            }
    
            /* disable RX FIFO level interrupt */
            sc.sc_irq_mask &= !GINTMSK_RXFLVLMSK;
            write_volatile(GINTMSK, sc.sc_irq_mask);
        }

        break;
    }

}

//TODO: TODO: Implement
fn dwc_otg_do_poll(sc: &mut dwc_otg_softc) {
    
    //TODO: Add a usb lock on this

    // struct dwc_otg_softc *sc = DWC_OTG_BUS2SC(bus);

	// USB_BUS_LOCK(&sc->sc_bus);
	// USB_BUS_SPIN_LOCK(&sc->sc_bus);
	dwc_otg_interrupt_poll_locked(sc);
	dwc_otg_interrupt_complete_locked(sc);
	// USB_BUS_SPIN_UNLOCK(&sc->sc_bus);
	// USB_BUS_UNLOCK(&sc->sc_bus);

}

fn dwc_otg_tx_fifo_reset(value: u32) {
    write_volatile(GRSTCTL, value);

    for _ in 0..16 {
        let temp = read_volatile(GRSTCTL);
        if(temp & (GRSTCTL_TXFFLSH | GRSTCTL_RXFFLSH)) == 0 {
            break;
        }
    }
}

fn dwc_otg_init_fifo(sc: &mut dwc_otg_softc) -> u32 {
    println!("| dwc_otg_init_fifo");

    let mut fifo_size = sc.sc_fifo_size;
    let fifo_regs = 4 * 16; // Why

    if fifo_size < fifo_regs {
        println!("| FIFO size too small: {}", fifo_size);
        return EINVAL;
    }

    /* subtract FIFO regs from total once */
	fifo_size -= fifo_regs;

	/* split equally for IN and OUT */
	fifo_size /= 2;

	/* Align to 4 bytes boundary (refer to PGM) */
	fifo_size &= !3;

    /* set global receive FIFO size */
    write_volatile(GRXFSIZ, fifo_size/4);

    let mut tx_start = fifo_size;

    /* reset active endpoints */
    sc.sc_active_rx_ep = 0;
    
    /* split equally for periodic and non-periodic */
    fifo_size /= 2;

    println!("| PTX/NPTX FIFO size: {}", fifo_size);
    /* align to 4 bytes boundary */
    fifo_size &= !3;
    write_volatile(GNPTXFSIZ, ((fifo_size / 4) << 16) | (tx_start / 4));

    for i in 0..sc.sc_host_ch_max {
        write_volatile(HCINTMSK(i as usize), HCINT_DEFAULT_MASK);
    }

    write_volatile(HPTXFSIZ, ((fifo_size / 4) << 16) | (tx_start / 4));
    /* reset host channel state */
    sc.sc_chan_state[0].allocated = 0;
    sc.sc_chan_state[0].wait_halted = 0;
    sc.sc_chan_state[0].hcint = 0;

    /* enable all host channel interrupts */
    write_volatile(HAINTMSK, (1 << sc.sc_host_ch_max) - 1);

    /* enable proper host channel interrupts */
    sc.sc_irq_mask |= GINTMSK_HCHINTMSK;
    sc.sc_irq_mask &= !GINTMSK_IEPINTMSK;
    write_volatile(GINTMSK, sc.sc_irq_mask);

    /* reset RX FIFO */
    dwc_otg_tx_fifo_reset(GRSTCTL_RXFFLSH);

    /* reset all TX FIFOs */
    dwc_otg_tx_fifo_reset((GRSTCTL_TXFIFO(0x10) | GRSTCTL_TXFFLSH) as u32);

    return 0;
}

fn dwc_otg_root_intr(sc: &mut dwc_otg_softc) {

    //TODO: USB_BUS_LOCK_ASSERT(&sc->sc_bus, MA_OWNED);
    sc.sc_hub_idata[0] = 0x02; /* we only have one port */

    //TODO: TODO: https://elixir.bootlin.com/freebsd/v14.2/source/sys/dev/usb/controller/dwc_otg.c#L3491
    // uhub_root_intr(&sc->sc_bus, sc->sc_hub_idata,
	//     sizeof(sc->sc_hub_idata));
    uhub_root_intr();
}

fn dwc_otg_vbus_interrupt(sc: &mut dwc_otg_softc, is_on: u8) {
    println!("| vbus = {}", is_on);

    if ((is_on != 0) || (sc.sc_mode == DWC_MODE_HOST)) {
		if (sc.sc_flags.status_vbus == 0) {
			sc.sc_flags.status_vbus = 1;

			/* complete root HUB interrupt endpoint */

			dwc_otg_root_intr(sc);
		}
	} else {
		if sc.sc_flags.status_vbus == 0 {
			sc.sc_flags.status_vbus = 0;
			sc.sc_flags.status_bus_reset = 0;
			sc.sc_flags.status_suspend = 0;
			sc.sc_flags.change_suspend = 0;
			sc.sc_flags.change_connect = 1;

			/* complete root HUB interrupt endpoint */

			dwc_otg_root_intr(sc);
		}
	}
}


pub fn dwc_otg_init(sc: &mut dwc_otg_softc) -> u32 {
    println!("| dwc_otg_init");

    sc.sc_mode = DWC_MODE_HOST;

    //TODO: Implement getting DMA memory -> 3854
    // if (usb_bus_mem_alloc_all(&sc->sc_bus,
	//     USB_GET_DMA_TAG(sc->sc_bus.parent), NULL)) {
	// 	return (ENOMEM);
	// }

    //3863 ???
    //device_set_ivars(sc->sc_bus.bdev, &sc->sc_bus);

    //TODO: TODO: Seems important Set up interrupts??
    // err = bus_setup_intr(sc->sc_bus.parent, sc->sc_irq_res,
	//     INTR_TYPE_TTY | INTR_MPSAFE, &dwc_otg_filter_interrupt,
	//     &dwc_otg_interrupt, sc, &sc->sc_intr_hdl);

    register_dwc_otg_interrupt_handler();

    // usb_callout_init_mtx(&sc->sc_timer,
	//     &sc->sc_bus.bus_mtx, 0);


	dwc_otg_clocks_on(sc);

    let ver = read_volatile(GSNPSID);
    println!("| DTC_OTG Version: 0x{:08x}", ver);

    let hprt = read_volatile(HPRT);
    println!("HPRT1 = 0x{:08x}", hprt);

    //disconnect
    write_volatile(DCTL, 1 << 1);
    usb_lock_mtx(1000 / 32);

    write_volatile(GRSTCTL, 1 << 0);
    //wait a little bit for block to reset
    usb_lock_mtx(1000/128);

    //GUSBCFG_FORCEHOSTMODE;
    let mut temp = 1<<29;

    sc.sc_phy_bits = 3;
    sc.sc_phy_type = 8; //TODO: Check, currently guessing based off of configuration

    // if (sc->sc_phy_type == 0)
	// 	sc->sc_phy_type = dwc_otg_phy_type + 1;
	// if (sc->sc_phy_bits == 0)
	// 	sc->sc_phy_bits = 16;

    //Case for UTMI+
    // DWC_OTG_WRITE_4(sc, DOTG_GUSBCFG,
    //     (sc->sc_phy_bits == 16 ? GUSBCFG_PHYIF : 0) |
    //     GUSBCFG_TRD_TIM_SET(5) | temp);
    // TODO: is GUSBCFG_TRD_TIM_SET(5) important? 3940

    if sc.sc_phy_type == 16 { temp |= 1 << 3; }
    write_volatile(GUSBCFG, (((5) & 15) << 10) | temp);
    write_volatile(GOTGCTL, 0);

    let temp = read_volatile(GLPMCFG);
    write_volatile(GLPMCFG, temp & !(1 << 30));

    //clear global nak
    write_volatile(DCTL, (1 << 10) | (1 << 8));
    //disable USB port
    write_volatile(PCGCCTL, 0xFFFFFFFF);

    //wait 10ms
    usb_lock_mtx(1000/100);

    //enable USB port
    write_volatile(PCGCCTL, 0); //Why do this??

    //wait 10ms
    usb_lock_mtx(1000/100);
    
    let temp = read_volatile(GHWCFG3);
    sc.sc_fifo_size = 4 * (temp >> 16); //?

    let temp = read_volatile(GHWCFG2);
    sc.sc_dev_ep_max = ((((temp) >> 10) & 15) + 1) as u8;
	// if (sc->sc_dev_ep_max > DWC_OTG_MAX_ENDPOINTS)
	// 	sc->sc_dev_ep_max = DWC_OTG_MAX_ENDPOINTS;

    sc.sc_host_ch_max = ((((temp) >> 14) & 15) + 1) as u8;
	// if (sc->sc_host_ch_max > DWC_OTG_MAX_CHANNELS)
	// 	sc->sc_host_ch_max = DWC_OTG_MAX_CHANNELS;
    
    let temp = read_volatile(GHWCFG4);
    sc.sc_dev_in_ep_max = ((((temp) >> 26) & 15) + 1)  as u8;

    println!("| fifo_size: {}, Device EPs: {}/{}, Host CHs: {}", sc.sc_fifo_size, sc.sc_dev_ep_max, sc.sc_dev_in_ep_max, sc.sc_host_ch_max);

    //TODO: TODO: SET UP FIFO
    /* setup FIFO */
	// if (dwc_otg_init_fifo(sc, sc->sc_mode)) {
	// 	USB_BUS_UNLOCK(&sc->sc_bus);
	// 	return (EINVAL);
	// }
    dwc_otg_init_fifo(sc); //Mode should be forced host

    //TODO: Set up interrupts

    sc.sc_irq_mask |= (1 << 12) | (1 << 13) | (1 << 24) | (1 << 31) | (1 << 11) | (1 << 2) | (1 << 30);
    write_volatile(GINTMSK, sc.sc_irq_mask);
    
    //hostmode
    //setup clocks
    let mut temp = read_volatile(HCFG);
    temp &= (1 << 2) | (0x00000003);
    temp |= (1 << 0); //?
    write_volatile(HCFG, temp);


    //only enable global interrupts
    println!("| Interrupts enabling");
    write_volatile(GAHBCFG, 1 << 0);
    println!("| Interrupts enabled");

	dwc_otg_clocks_off(sc);

    //read initial VBUS state
    let temp = read_volatile(GOTGCTL);
    println!("| VBUS state: 0x{:08x}", temp);


    //TODO: TODO: This seems important
    // dwc_otg_vbus_interrupt(sc,
	    // (temp & (GOTGCTL_ASESVLD | GOTGCTL_BSESVLD)) ? 1 : 0);
    let temp_interrupt = (temp & (GOTGCTL_ASESVLD | GOTGCTL_BSESVLD)) != 0;
    dwc_otg_vbus_interrupt(sc, temp_interrupt as u8);

    //USB_BUS_UNLOCK(&sc->sc_bus);

    //catch any lost interrupts
    dwc_otg_do_poll(sc);
    

    return 0;
}


//time = 1000 for 1 second //Maybe
fn usb_pause_mtx(time: usize) {
    let start_time = system_timer::get_time();
    while(((system_timer::get_time() - start_time) * 1000) / (system_timer::get_freq())) < (time as u64) {}
}


//time = 1000 for 1 second //Maybe
//TODO: Add a usb lock on this
fn usb_lock_mtx(time: usize) {
    let start_time = system_timer::get_time();
    while(((system_timer::get_time() - start_time) * 1000) / (system_timer::get_freq())) < (time as u64) {}
}

fn read_volatile(reg: usize) -> u32 {
    unsafe { core::ptr::read_volatile((dwc_otg_driver.base_addr + reg) as *mut u32) }
}
fn write_volatile(reg: usize, val: u32) {
    unsafe { core::ptr::write_volatile((dwc_otg_driver.base_addr + reg) as *mut u32, val) }
}

pub fn dwc_otg_initialize_controller(base_addr: *mut()) {
    unsafe {
        dwc_otg_driver = DWC_OTG::init(base_addr);
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct dwc_otg_chan_state {
    allocated: u16,
    wait_halted: u16,
    hcint: u32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct dwc_otg_flags {
    change_connect: u8,
    change_suspend: u8,
    change_reset: u8,
    change_enabled: u8,
    change_over_current: u8,
    status_suspend: u8,
    status_vbus: u8,
    status_bus_reset: u8,
    status_high_speed: u8,
    status_low_speed: u8,
    status_device_mode: u8,
    self_powered: u8,
    clocks_off: u8,
    port_powered: u8,
    port_enabled: u8,
    port_over_current: u8,
    d_pulled_up: u8,
}
 
impl dwc_otg_flags {
    pub const fn new() -> Self {
        dwc_otg_flags {
            change_connect: 0,
            change_suspend: 0,
            change_reset: 0,
            change_enabled: 0,
            change_over_current: 0,
            status_suspend: 0,
            status_vbus: 0,
            status_bus_reset: 0,
            status_high_speed: 0,
            status_low_speed: 0,
            status_device_mode: 0,
            self_powered: 0,
            clocks_off: 1,
            port_powered: 0,
            port_enabled: 0,
            port_over_current: 0,
            d_pulled_up: 0
        }
    }
}



#[repr(C)]
#[derive(Clone, Copy)]
pub union dwc_otg_hub_temp {
    pub wValue: u16,
    pub ps: usb_port_status,
}

impl dwc_otg_hub_temp {
    pub const fn new() -> Self {
        dwc_otg_hub_temp {
            wValue: 0
        }
    }
}


#[repr(C)]
#[derive(Clone, Copy)]
pub struct dwc_otg_softc {
    // struct usb_bus sc_bus;
    pub sc_hub_temp: dwc_otg_hub_temp,
    // struct dwc_otg_profile sc_hw_ep_profile[DWC_OTG_MAX_ENDPOINTS];
    // struct dwc_otg_tt_info sc_tt_info[DWC_OTG_MAX_DEVICES];
    // struct usb_callout sc_timer;

    // struct usb_device *sc_devices[DWC_OTG_MAX_DEVICES];
    // struct resource *sc_io_res;
    // struct resource *sc_irq_res;
    // void   *sc_intr_hdl;
    // bus_size_t sc_io_size;
    // bus_space_tag_t sc_io_tag;
    // bus_space_handle_t sc_io_hdl;

    // uint32_t sc_bounce_buffer[MAX(512 * DWC_OTG_MAX_TXP, 1024) / 4];

    pub sc_fifo_size: u32,
    pub sc_irq_mask: u32,
    pub sc_last_rx_status: u32,
    pub sc_out_ctl: [u32; DWC_OTG_MAX_ENDPOINTS],
    pub sc_in_ctl: [u32; DWC_OTG_MAX_ENDPOINTS],
    pub sc_chan_state: [dwc_otg_chan_state; DWC_OTG_MAX_CHANNELS],
    pub sc_tmr_val: u32,
    pub sc_hprt_val: u32,
    pub sc_xfer_complete: u32,

    pub sc_current_rx_bytes: u16,
    pub sc_current_rx_fifo: u16,

    pub sc_active_rx_ep: u16,
    pub sc_last_frame_num: u16,

    pub sc_phy_type: u8,
    pub sc_phy_bits: u8,

    pub sc_timer_active: u8,
    pub sc_dev_ep_max: u8,
    pub sc_dev_in_ep_max: u8,
    pub sc_host_ch_max: u8,
    pub sc_needsof: u8,
    pub sc_rt_addr: u8, // root HUB address
    pub sc_conf: u8,    // root HUB config
    pub sc_mode: u8,    // mode of operation
    pub sc_hub_idata: [u8; 1],

    pub sc_flags: dwc_otg_flags,
}

impl dwc_otg_softc {
    pub const fn new() ->  Self {
        let mut this = dwc_otg_softc {
            sc_hub_temp: dwc_otg_hub_temp::new(),
            sc_fifo_size: 0,
            sc_irq_mask: 0,
            sc_last_rx_status: 0,
            sc_out_ctl: [0; DWC_OTG_MAX_ENDPOINTS],
            sc_in_ctl: [0; DWC_OTG_MAX_ENDPOINTS],
            sc_chan_state: [dwc_otg_chan_state { wait_halted: 0, allocated: 0, hcint: 0 }; DWC_OTG_MAX_CHANNELS],
            sc_tmr_val: 0,
            sc_hprt_val: 0,
            sc_xfer_complete: 0,
            sc_current_rx_bytes: 0,
            sc_current_rx_fifo: 0,
            sc_active_rx_ep: 0,
            sc_last_frame_num: 0,
            sc_phy_type: 0,
            sc_phy_bits: 0,
            sc_timer_active: 0,
            sc_dev_ep_max: 0,
            sc_dev_in_ep_max: 0,
            sc_host_ch_max: 0,
            sc_needsof: 0,
            sc_rt_addr: 0, // root HUB address
            sc_conf: 0,    // root HUB config
            sc_mode: 0,    // mode of operation
            sc_hub_idata: [0; 1],
            sc_flags: dwc_otg_flags::new(),
        };
        this
    }

    pub fn init(&mut self) {
        self.sc_fifo_size = 0;
        self.sc_irq_mask = 0;
        self.sc_last_rx_status = 0;
        self.sc_out_ctl = [0; DWC_OTG_MAX_ENDPOINTS];
        self.sc_in_ctl = [0; DWC_OTG_MAX_ENDPOINTS];
        self.sc_chan_state = [dwc_otg_chan_state { wait_halted: 0, allocated: 0, hcint: 0 }; DWC_OTG_MAX_CHANNELS];
        self.sc_tmr_val = 0;
        self.sc_hprt_val = 0;
        self.sc_xfer_complete = 0;
        self.sc_current_rx_bytes = 0;
        self.sc_current_rx_fifo = 0;
        self.sc_active_rx_ep = 0;
        self.sc_last_frame_num = 0;
        self.sc_phy_type = 0;
        self.sc_phy_bits = 0;
        self.sc_timer_active = 0;
        self.sc_dev_ep_max = 0;
        self.sc_dev_in_ep_max = 0;
        self.sc_host_ch_max = 0;
        self.sc_needsof = 0;
        self.sc_rt_addr = 0; // root HUB address
        self.sc_conf = 0;    // root HUB config
        self.sc_mode = 0;    // mode of operation
        self.sc_hub_idata = [0; 1];
        self.sc_flags = dwc_otg_flags::new();
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct dwc_otg_config_desc {
    pub confd: usb_config_descriptor,
    pub ifcd: usb_interface_descriptor,
    pub endpd: usb_endpoint_descriptor,
}


/*------------------------------------------------------------------------*
 * DWC OTG root control support
 *------------------------------------------------------------------------*
 * Simulate a hardware HUB by handling all the necessary requests.
 *------------------------------------------------------------------------*/

pub static dwc_otg_devd: usb_device_descriptor = usb_device_descriptor {
    bLength: core::mem::size_of::<usb_device_descriptor>() as u8,
    bDescriptorType: UDESC_DEVICE,
    bcdUSB: 0x0200, // Equivalent to {0x00, 0x02} in little-endian
    bDeviceClass: UDCLASS_HUB,
    bDeviceSubClass: UDSUBCLASS_HUB,
    bDeviceProtocol: UDPROTO_HSHUBSTT,
    bMaxPacketSize: 64,
    idVendor: 0,
    idProduct: 0,
    bcdDevice: 0x0100, // Equivalent to {0x00, 0x01} in little-endian
    iManufacturer: 1,
    iProduct: 2,
    iSerialNumber: 0, 
    bNumConfigurations: 1,
};

pub static dwc_otg_confd: dwc_otg_config_desc = dwc_otg_config_desc {
    confd: usb_config_descriptor {
        bLength: core::mem::size_of::<usb_config_descriptor>() as u8,
        bDescriptorType: UDESC_CONFIG,
        wTotalLength: core::mem::size_of::<dwc_otg_config_desc>() as u16, // Equivalent to HSETW
        bNumInterface: 1,
        bConfigurationValue: 1,
        iConfiguration: 0,
        bmAttributes: UC_SELF_POWERED,
        bMaxPower: 0,
    },
    ifcd: usb_interface_descriptor {
        bLength: core::mem::size_of::<usb_interface_descriptor>() as u8,
        bDescriptorType: UDESC_INTERFACE,
        bNumEndpoints: 1,
        bInterfaceClass: UICLASS_HUB,
        bInterfaceSubClass: UISUBCLASS_HUB,
        bInterfaceNumber: 0,
        bAlternateSetting: 0,
        bInterfaceProtocol: 0,
        iInterface: 0, // Missing in original C structure, initialized to 0
    },
    endpd: usb_endpoint_descriptor {
        bLength: core::mem::size_of::<usb_endpoint_descriptor>() as u8,
        bDescriptorType: UDESC_ENDPOINT,
        bEndpointAddress: UE_DIR_IN | DWC_OTG_INTR_ENDPT,
        bmAttributes: UE_INTERRUPT,
        wMaxPacketSize: 8, // Equivalent to HSETW
        bInterval: 255,
    },
};

// Helper function to replace HSETW macro
pub const fn hsetw(value: u16) -> u16 {
    value // Rust handles endian conversions automatically if needed
}

pub static dwc_otg_hubd: usb_hub_descriptor_min = usb_hub_descriptor_min {
    bDescLength: core::mem::size_of::<usb_hub_descriptor_min>() as u8,
    bDescriptorType: UDESC_HUB,
    bNbrPorts: 1,
    wHubCharacteristics: hsetw(UHD_PWR_NO_SWITCH | UHD_OC_INDIVIDUAL), // Equivalent to HSETW
    bPwrOn2PwrGood: 50,
    bHubContrCurrent: 0,
    DeviceRemovable: [0], // Port is removable
    PortPowerCtrlMask: [0], // No power switching
};


pub const DWC_OTG_MAX_ENDPOINTS: usize = 16; // Update as needed
pub const DWC_OTG_MAX_CHANNELS: usize = 8;   // Update as needed

const DWC_OTG_PHY_ULPI: u8 = 1;
const DWC_OTG_PHY_HSIC: u8 = 2;
const DWC_OTG_PHY_INTERNAL: u8 = 3;
const DWC_OTG_PHY_UTMI: u8 = 4;

const DWC_MODE_OTG: u8 = 0;
const DWC_MODE_DEVICE: u8 = 1;
const DWC_MODE_HOST: u8 = 2;


const GOTGCTL: usize = 0x000;
const GOTGINT: usize = 0x004;
const GAHBCFG: usize = 0x008;
const GUSBCFG: usize = 0x00C;
const GRSTCTL: usize = 0x010;
const GINTSTS: usize = 0x014;
const GINTMSK: usize = 0x018;
const GRXSTSR: usize = 0x01C;
const GRXSTSP: usize = 0x020;
const GRXFSIZ: usize = 0x024;
const GNPTXFSIZ: usize = 0x028;
const GNPTXSTS: usize = 0x02C;
const GI2CCTL: usize = 0x030;
const GPVNDCTL: usize = 0x034;
const GGPIO: usize = 0x038;
const GUID: usize = 0x03C;
const GSNPSID: usize = 0x040;
const GHWCFG1: usize = 0x044;
const GHWCFG2: usize = 0x048;
const GHWCFG3: usize = 0x04C;
const GHWCFG4: usize = 0x050;
const GLPMCFG: usize = 0x054;
const GPWRDN: usize = 0x058;
const GDFIFOCFG: usize = 0x05C;
const GADPCTL: usize = 0x060;

const HPTXFSIZ: usize = 0x100;
//DPTXFSIZn not defined
const fn DIEPTXF(n: usize) -> usize { 0x104 + 4 * n }   

const HCFG: usize = 0x400;
const HFIR: usize = 0x404;
const HFNUM: usize = 0x408;
const HPTXSTS: usize = 0x410;
const HAINT: usize = 0x414;
const HAINTMSK: usize = 0x418;
const HFLBADDR: usize = 0x41C;
const HPRT: usize = 0x440;


const fn HCCHAR(n: usize) -> usize { 0x500 + 0x20 * n }
const fn HCSPLT(n: usize) -> usize { 0x504 + 0x20 * n }
const fn HCINT(n: usize) -> usize { 0x508 + 0x20 * n }
const fn HCINTMSK(n: usize) -> usize { 0x50C + 0x20 * n }
const fn HCTSIZ(n: usize) -> usize { 0x510 + 0x20 * n }
const fn HCDMA(n: usize) -> usize { 0x514 + 0x20 * n }
const fn HCDMAB(n: usize) -> usize { 0x51C + 0x20 * n + 4 }

const DCFG: usize = 0x800;
const DCTL: usize = 0x804;
const DSTS: usize = 0x808;

pub const fn DIEPINT(ep: u32) -> u32 { 0x908 + (ep) * 32 }

const PCGCCTL: usize = 0x0E00;

struct DWC_OTG {
    base_addr: usize,
}

impl DWC_OTG {
    pub unsafe fn init(base_addr: *mut ()) -> Self {
        Self { base_addr: base_addr as usize }
    }
}