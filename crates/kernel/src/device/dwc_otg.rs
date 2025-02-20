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

// mod dwc_otgreg;
use crate::device::dwc_otgreg::*;
use super::system_timer::{self, SYSTEM_TIMER};
pub static mut dwc_otg_driver: DWC_OTG = DWC_OTG { base_addr: 0 };

//TODO: TODO: Implement
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


pub fn dwc_otg_init(sc: &mut dwc_otg_softc) -> u32 {
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

    sc.sc_irq_mask = 0;
    sc.sc_irq_mask |= (1 << 12) | (1 << 13) | (1 << 24) | (1 << 31) | (1 << 11) | (1 << 2) | (1 << 30);
    write_volatile(GINTMSK, sc.sc_irq_mask);

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

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct dwc_otg_chan_state {
    allocated: u16,
    wait_halted: u16,
    hcint: u32,
}

// struct dwc_otg_flags {
// 	uint8_t	change_connect:1;
// 	uint8_t	change_suspend:1;
// 	uint8_t change_reset:1;
// 	uint8_t change_enabled:1;
// 	uint8_t change_over_current:1;
// 	uint8_t	status_suspend:1;	/* set if suspended */
// 	uint8_t	status_vbus:1;		/* set if present */
// 	uint8_t	status_bus_reset:1;	/* set if reset complete */
// 	uint8_t	status_high_speed:1;	/* set if High Speed is selected */
// 	uint8_t	status_low_speed:1;	/* set if Low Speed is selected */
// 	uint8_t status_device_mode:1;	/* set if device mode */
// 	uint8_t	self_powered:1;
// 	uint8_t	clocks_off:1;
// 	uint8_t	port_powered:1;
// 	uint8_t	port_enabled:1;
// 	uint8_t port_over_current:1;
// 	uint8_t	d_pulled_up:1;
// }

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct dwc_otg_softc {
    // struct usb_bus sc_bus;
    // union dwc_otg_hub_temp sc_hub_temp;
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

    // pub sc_flags: DwcOtgFlags,
}

impl dwc_otg_softc {
    pub fn new() ->  Self {
        Self::default()
    }
}


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

const PCGCCTL: usize = 0x0E00;

struct DWC_OTG {
    base_addr: usize,
}

impl DWC_OTG {
    pub unsafe fn init(base_addr: *mut ()) -> Self {
        Self { base_addr: base_addr as usize }
    }
}