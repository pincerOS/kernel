/*-
 * SPDX-License-Identifier: BSD-2-Clause
 *
 * Copyright (c) 2015 Daisuke Aoyama. All rights reserved.
 * Copyright (c) 2012-2015 Hans Petter Selasky. All rights reserved.
 * Copyright (c) 2010-2011 Aleksandr Rybalko. All rights reserved.
 *
 * Modified in 2025 for Rust compatibility by Aaron Lo <aaronlo0929@gmail.com>
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

use super::system_timer::{self, SYSTEM_TIMER};
pub static mut dwc_otg_driver: DWC_OTG = DWC_OTG { base_addr: 0 };

fn dwc_otg_do_poll() {
    
    //TODO: Add a usb lock on this

    // struct dwc_otg_softc *sc = DWC_OTG_BUS2SC(bus);

	// USB_BUS_LOCK(&sc->sc_bus);
	// USB_BUS_SPIN_LOCK(&sc->sc_bus);
	// dwc_otg_interrupt_poll_locked(sc);
	// dwc_otg_interrupt_complete_locked(sc);
	// USB_BUS_SPIN_UNLOCK(&sc->sc_bus);
	// USB_BUS_UNLOCK(&sc->sc_bus);
}


pub fn dtc_otg_init() -> u32 {
    println!("| dwc_otg_init");

    //TODO: Implement getting DMA memory -> 3854
    // if (usb_bus_mem_alloc_all(&sc->sc_bus,
	//     USB_GET_DMA_TAG(sc->sc_bus.parent), NULL)) {
	// 	return (ENOMEM);
	// }

    //3863 ???
    //device_set_ivars(sc->sc_bus.bdev, &sc->sc_bus);

    //TODO: Set up interrupts??
    // err = bus_setup_intr(sc->sc_bus.parent, sc->sc_irq_res,
	//     INTR_TYPE_TTY | INTR_MPSAFE, &dwc_otg_filter_interrupt,
	//     &dwc_otg_interrupt, sc, &sc->sc_intr_hdl);

    // usb_callout_init_mtx(&sc->sc_timer,
	//     &sc->sc_bus.bus_mtx, 0);


    ///* turn on clocks */
	// dwc_otg_clocks_on(sc);
    //TOOD: Turn on the clock

    let ver = read_volatile(GSNPSID);
    println!("| DTC_OTG Version: 0x{:08x}", ver);

    //disconnect
    write_volatile(DCTL, 1 << 1);
    usb_lock_mtx(1000 / 32);

    write_volatile(GRSTCTL, 1 << 0);
    //wait a little bit for block to reset
    usb_lock_mtx(1000/128);

    //GUSBCFG_FORCEHOSTMODE;
    let mut temp = 1<<29;

    let sc_phy_type = 3;
    let sc_phy_bits = 8; //TODO: Check, currently guessing based off of configuration

    // if (sc->sc_phy_type == 0)
	// 	sc->sc_phy_type = dwc_otg_phy_type + 1;
	// if (sc->sc_phy_bits == 0)
	// 	sc->sc_phy_bits = 16;

    //Case for UTMI+
    // DWC_OTG_WRITE_4(sc, DOTG_GUSBCFG,
    //     (sc->sc_phy_bits == 16 ? GUSBCFG_PHYIF : 0) |
    //     GUSBCFG_TRD_TIM_SET(5) | temp);
    // TODO: is GUSBCFG_TRD_TIM_SET(5) important? 3940

    if sc_phy_bits == 16 { temp |= 1 << 3; }
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
    let sc_fifo_size = 4 * (temp >> 16); //?

    let temp = read_volatile(GHWCFG2);
    let sc_dev_ep_max = ((((temp) >> 10) & 15) + 1);
	// if (sc->sc_dev_ep_max > DWC_OTG_MAX_ENDPOINTS)
	// 	sc->sc_dev_ep_max = DWC_OTG_MAX_ENDPOINTS;

    let sc_host_ch_max = ((((temp) >> 14) & 15) + 1);
	// if (sc->sc_host_ch_max > DWC_OTG_MAX_CHANNELS)
	// 	sc->sc_host_ch_max = DWC_OTG_MAX_CHANNELS;
    
    let temp = read_volatile(GHWCFG4);
    let sc_dev_in_ep_max = ((((temp) >> 26) & 15) + 1);

    println!("| fifo_size: {}, Device EPs: {}/{}, Host CHs: {}", sc_fifo_size, sc_dev_ep_max, sc_dev_in_ep_max, sc_host_ch_max);

    //TODO: TODO: SET UP FIFO
    /* setup FIFO */
	// if (dwc_otg_init_fifo(sc, sc->sc_mode)) {
	// 	USB_BUS_UNLOCK(&sc->sc_bus);
	// 	return (EINVAL);
	// }

    //TODO: Set up interrupts
    let sc_irq_mask = (1 << 12) | (1 << 13) | (1 << 24) | (1 << 31) | (1 << 11) | (1 << 2) | (1 << 30);
    write_volatile(GINTMSK, sc_irq_mask);

    //hostmode
    //setup clocks
    let mut temp = read_volatile(HCFG);
    temp &= (1 << 2) | (0x00000003);
    temp |= (1 << 0); //?
    write_volatile(HCFG, temp);

    //only enable global interrupts
    write_volatile(GAHBCFG, 1 << 0);

    //TODO: turn off the clocks
	// dwc_otg_clocks_off(sc);

    //read initial VBUS state
    let temp = read_volatile(GOTGCTL);
    println!("| VBUS state: 0x{:08x}", temp);


    //TODO: TODO: This seems important
    // dwc_otg_vbus_interrupt(sc,
	    // (temp & (GOTGCTL_ASESVLD | GOTGCTL_BSESVLD)) ? 1 : 0);

    //USB_BUS_UNLOCK(&sc->sc_bus);

    //catch any lost interrupts
    dwc_otg_do_poll();
    

    return 0;
}

//time = 1000 for 1 second //Maybe
//TODO: Add a usb lock on this
fn usb_lock_mtx(time: usize) {
    let start_time = system_timer::get_time();
    while((system_timer::get_time() - start_time) / (system_timer::get_freq() / 1000)) < (time as u64) { }
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
const HPRT: usize = 0x440;

const DCFG: usize = 0x800;
const DCTL: usize = 0x804;
const DSTS: usize = 0x808;

const PCGCCTL: usize = 0x0E00;

struct DWC_OTG {
    base_addr: usize,
}

impl DWC_OTG {
    pub unsafe fn init(base_addr: *mut ()) -> Self {
        Self { base_addr: base_addr as usize }
    }
}