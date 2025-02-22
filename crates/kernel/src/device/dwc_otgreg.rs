/*-
 * SPDX-License-Identifier: BSD-2-Clause
 *
 * Copyright (c) 2010,2011 Aleksandr Rybalko. All rights reserved.
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

pub const EINVAL: u32 = 22;
pub const FILTER_HANDLED: u32 = 0x02;
pub const FILTER_SCHEDULE_THREAD: u32 = 0x04;

pub const fn DOTG_DFIFO(n: u32) -> u32 { 0x1000 + (n) * 0x1000}

pub const DWC_OTG_MSK_GINT_THREAD_IRQ: u32 = (GINTSTS_USBRST | GINTSTS_ENUMDONE | GINTSTS_PRTINT |	GINTSTS_WKUPINT | GINTSTS_USBSUSP | GINTMSK_OTGINTMSK |	GINTSTS_SESSREQINT);

pub const fn GRSTCTL_TXFIFO(n: u32) -> u32 { ((n) & 31) << 6}
pub const GRSTCTL_TXFFLSH: u32 = 1 << 5;
pub const GRSTCTL_RXFFLSH: u32 = 1 << 4;

pub const GINTSTS_WKUPINT: u32       = 1 << 31;
pub const GINTSTS_SESSREQINT: u32    = 1 << 30;
pub const GINTSTS_DISCONNINT: u32    = 1 << 29;
pub const GINTSTS_CONIDSTSCHNG: u32  = 1 << 28;
pub const GINTSTS_LPM: u32           = 1 << 27;
pub const GINTSTS_PTXFEMP: u32       = 1 << 26;
pub const GINTSTS_HCHINT: u32        = 1 << 25;
pub const GINTSTS_PRTINT: u32        = 1 << 24;
pub const GINTSTS_RESETDET: u32      = 1 << 23;
pub const GINTSTS_FETSUSP: u32       = 1 << 22;
pub const GINTSTS_INCOMPLP: u32      = 1 << 21;
pub const GINTSTS_INCOMPISOIN: u32   = 1 << 20;
pub const GINTSTS_OEPINT: u32        = 1 << 19;
pub const GINTSTS_IEPINT: u32        = 1 << 18;
pub const GINTSTS_EPMIS: u32         = 1 << 17;
pub const GINTSTS_RESTORE_DONE: u32  = 1 << 16;
pub const GINTSTS_EOPF: u32          = 1 << 15;
pub const GINTSTS_ISOOUTDROP: u32    = 1 << 14;
pub const GINTSTS_ENUMDONE: u32      = 1 << 13;
pub const GINTSTS_USBRST: u32        = 1 << 12;
pub const GINTSTS_USBSUSP: u32       = 1 << 11;
pub const GINTSTS_ERLYSUSP: u32      = 1 << 10;
pub const GINTSTS_I2CINT: u32        = 1 << 9;
pub const GINTSTS_ULPICKINT: u32     = 1 << 8;
pub const GINTSTS_GOUTNAKEFF: u32    = 1 << 7;
pub const GINTSTS_GINNAKEFF: u32     = 1 << 6;
pub const GINTSTS_NPTXFEMP: u32      = 1 << 5;
pub const GINTSTS_RXFLVL: u32        = 1 << 4;
pub const GINTSTS_SOF: u32           = 1 << 3;
pub const GINTSTS_OTGINT: u32        = 1 << 2;
pub const GINTSTS_MODEMIS: u32       = 1 << 1;
pub const GINTSTS_CURMOD: u32        = 1 << 0;


pub const GINTMSK_WKUPINTMSK: u32      = 1 << 31;
pub const GINTMSK_SESSREQINTMSK: u32   = 1 << 30;
pub const GINTMSK_DISCONNINTMSK: u32   = 1 << 29;
pub const GINTMSK_CONIDSTSCHNGMSK: u32 = 1 << 28;
pub const GINTMSK_PTXFEMPMSK: u32      = 1 << 26;
pub const GINTMSK_HCHINTMSK: u32       = 1 << 25;
pub const GINTMSK_PRTINTMSK: u32       = 1 << 24;
pub const GINTMSK_FETSUSPMSK: u32      = 1 << 22;
pub const GINTMSK_INCOMPLPMSK: u32     = 1 << 21;
pub const GINTMSK_INCOMPISOINMSK: u32  = 1 << 20;
pub const GINTMSK_OEPINTMSK: u32       = 1 << 19;
pub const GINTMSK_IEPINTMSK: u32       = 1 << 18;
pub const GINTMSK_EPMISMSK: u32        = 1 << 17;
pub const GINTMSK_EOPFMSK: u32         = 1 << 15;
pub const GINTMSK_ISOOUTDROPMSK: u32   = 1 << 14;
pub const GINTMSK_ENUMDONEMSK: u32     = 1 << 13;
pub const GINTMSK_USBRSTMSK: u32       = 1 << 12;
pub const GINTMSK_USBSUSPMSK: u32      = 1 << 11;
pub const GINTMSK_ERLYSUSPMSK: u32     = 1 << 10;
pub const GINTMSK_I2CINTMSK: u32       = 1 << 9;
pub const GINTMSK_ULPICKINTMSK: u32    = 1 << 8;
pub const GINTMSK_GOUTNAKEFFMSK: u32   = 1 << 7;
pub const GINTMSK_GINNAKEFFMSK: u32    = 1 << 6;
pub const GINTMSK_NPTXFEMPMSK: u32     = 1 << 5;
pub const GINTMSK_RXFLVLMSK: u32       = 1 << 4;
pub const GINTMSK_SOFMSK: u32          = 1 << 3;
pub const GINTMSK_OTGINTMSK: u32       = 1 << 2;
pub const GINTMSK_MODEMISMSK: u32      = 1 << 1;
pub const GINTMSK_CURMODMSK: u32       = 1 << 0;

// Define constants using Rust's const
pub const GRXSTSRD_FN_MASK: u32 = 0x01E00000;
pub const GRXSTSRD_FN_SHIFT: u32 = 21;
pub const GRXSTSRD_PKTSTS_MASK: u32 = 0x001E0000;
pub const GRXSTSRD_PKTSTS_SHIFT: u32 = 17;
pub const GRXSTSRH_IN_DATA: u32 = 2 << 17;
pub const GRXSTSRH_IN_COMPLETE: u32 = 3 << 17;
pub const GRXSTSRH_DT_ERROR: u32 = 5 << 17;
pub const GRXSTSRH_HALTED: u32 = 7 << 17;
pub const GRXSTSRD_GLOB_OUT_NAK: u32 = 1 << 17;
pub const GRXSTSRD_OUT_DATA: u32 = 2 << 17;
pub const GRXSTSRD_OUT_COMPLETE: u32 = 3 << 17;
pub const GRXSTSRD_STP_COMPLETE: u32 = 4 << 17;
pub const GRXSTSRD_STP_DATA: u32 = 6 << 17;
pub const GRXSTSRD_DPID_MASK: u32 = 0x00018000;
pub const GRXSTSRD_DPID_SHIFT: u32 = 15;
pub const GRXSTSRD_DPID_DATA0: u32 = 0 << 15;
pub const GRXSTSRD_DPID_DATA1: u32 = 2 << 15;
pub const GRXSTSRD_DPID_DATA2: u32 = 1 << 15;
pub const GRXSTSRD_DPID_MDATA: u32 = 3 << 15;   
pub const GRXSTSRD_BCNT_MASK: u32 = 0x00007FF0;
pub const GRXSTSRD_BCNT_SHIFT: u32 = 4;
pub const GRXSTSRD_CHNUM_MASK: u32 = 0x0000000F;
pub const GRXSTSRD_CHNUM_SHIFT: u32 = 0;

pub const fn GRXSTSRD_CHNUM_GET(x: u32) -> u32 { x & 15}
pub const fn GRXSTSRD_BCNT_GET(x: u32) -> u32 { (x >> 4) & 0x7FF}

pub const GOTGCTL_BSESVLD: u32 = 1 << 19;
pub const GOTGCTL_ASESVLD: u32 = 1 << 18;

pub const DCTL_PWRONPRGDONE: u32 = 1 << 11;
pub const DCTL_CGOUTNAK: u32 = 1 << 10;
pub const DCTL_SGOUTNAK: u32 = 1 << 9;
pub const DCTL_CGNPINNAK: u32 = 1 << 8;
pub const DCTL_SGNPINNAK: u32 = 1 << 7;
pub const DCTL_TSTCTL_SHIFT: u32 = 4;
pub const DCTL_TSTCTL_MASK: u32 = 0x00000070;
pub const DCTL_GOUTNAKSTS: u32 = 1 << 3;
pub const DCTL_GNPINNAKSTS: u32 = 1 << 2;
pub const DCTL_SFTDISCON: u32 = 1 << 1;
pub const DCTL_RMTWKUPSIG: u32 = 1 << 0;

pub const HPRT_PRTSPD_SHIFT: u32      = 17;
pub const HPRT_PRTSPD_MASK: u32       = 0x00060000;
pub const HPRT_PRTSPD_HIGH: u32       = 0;
pub const HPRT_PRTSPD_FULL: u32       = 1;
pub const HPRT_PRTSPD_LOW: u32        = 2;
pub const HPRT_PRTTSTCTL_SHIFT: u32   = 13;
pub const HPRT_PRTTSTCTL_MASK: u32    = 0x0001e000;
pub const HPRT_PRTPWR: u32            = 1 << 12;
pub const HPRT_PRTLNSTS_SHIFT: u32    = 10;
pub const HPRT_PRTLNSTS_MASK: u32     = 0x00000c00;
pub const HPRT_PRTRST: u32            = 1 << 8;
pub const HPRT_PRTSUSP: u32           = 1 << 7;
pub const HPRT_PRTRES: u32            = 1 << 6;
pub const HPRT_PRTOVRCURRCHNG: u32    = 1 << 5;
pub const HPRT_PRTOVRCURRACT: u32     = 1 << 4;
pub const HPRT_PRTENCHNG: u32         = 1 << 3;
pub const HPRT_PRTENA: u32            = 1 << 2;
pub const HPRT_PRTCONNDET: u32        = 1 << 1;
pub const HPRT_PRTCONNSTS: u32        = 1 << 0;

pub const HFIR_RELOADCTRL: u32    = 1 << 16;
pub const HFIR_FRINT_SHIFT: u32   = 0;
pub const HFIR_FRINT_MASK: u32    = 0x0000ffff;


pub const HCINT_DEFAULT_MASK: u32 = 
    HCINT_STALL | HCINT_BBLERR |
    HCINT_XACTERR | HCINT_NAK | HCINT_ACK | HCINT_NYET |
    HCINT_CHHLTD | HCINT_FRMOVRUN |
    HCINT_DATATGLERR;

pub const HCINT_SOFTWARE_ONLY: u32 = 1 << 20;
pub const HCINT_DATATGLERR: u32 = 1 << 10;
pub const HCINT_FRMOVRUN: u32 = 1 << 9;
pub const HCINT_BBLERR: u32 = 1 << 8;
pub const HCINT_XACTERR: u32 = 1 << 7;
pub const HCINT_NYET: u32 = 1 << 6;
pub const HCINT_ACK: u32 = 1 << 5;
pub const HCINT_NAK: u32 = 1 << 4;
pub const HCINT_STALL: u32 = 1 << 3;
pub const HCINT_AHBERR: u32 = 1 << 2;
pub const HCINT_CHHLTD: u32 = 1 << 1;
pub const HCINT_XFERCOMPL: u32 = 1 << 0;
