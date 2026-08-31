/*-
 * SPDX-License-Identifier: BSD-2-Clause
 *
 * Copyright (c) 2010,2011 Aleksandr Rybalko. All rights reserved.
 *
 * Converted to Rust by Aaron Lo
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

pub const DOTG_GOTGCTL: usize = 0x0000;
pub const DOTG_GOTGINT: usize = 0x0004;
pub const DOTG_GAHBCFG: usize = 0x0008;
pub const DOTG_GUSBCFG: usize = 0x000C;
pub const DOTG_GRSTCTL: usize = 0x0010;
pub const DOTG_GINTSTS: usize = 0x0014;
pub const DOTG_GINTMSK: usize = 0x0018;
pub const DOTG_GRXSTSRD: usize = 0x001C;
pub const DOTG_GRXSTSRH: usize = 0x001C;
pub const DOTG_GRXSTSPD: usize = 0x0020;
pub const DOTG_GRXSTSPH: usize = 0x0020;
pub const DOTG_GRXFSIZ: usize = 0x0024;
pub const DOTG_GNPTXFSIZ: usize = 0x0028;
pub const DOTG_GNPTXSTS: usize = 0x002C;
pub const DOTG_GI2CCTL: usize = 0x0030;
pub const DOTG_GPVNDCTL: usize = 0x0034;
pub const DOTG_GGPIO: usize = 0x0038;
pub const DOTG_GUID: usize = 0x003C;
pub const DOTG_GSNPSID: usize = 0x0040;
pub const DOTG_GSNPSID_REV_2_80a: usize = 0x4f54280a; // RPi model B/RPi2
pub const DOTG_GSNPSID_REV_3_10a: usize = 0x4f54310a; // ODROID-C1
pub const DOTG_GHWCFG1: usize = 0x0044;
pub const DOTG_GHWCFG2: usize = 0x0048;
pub const DOTG_GHWCFG3: usize = 0x004C;
pub const DOTG_GHWCFG4: usize = 0x0050;
pub const DOTG_GLPMCFG: usize = 0x0054;
pub const DOTG_GPWRDN: usize = 0x0058;
pub const DOTG_GDFIFOCFG: usize = 0x005C;
pub const DOTG_GADPCTL: usize = 0x0060;
pub const DOTG_HPTXFSIZ: usize = 0x0100;
// start from 0x104, but fifo0 not exists
pub const fn DOTG_DPTXFSIZ(fifo: usize) -> usize {
    0x0100 + (4 * fifo)
}
pub const fn DOTG_DIEPTXF(fifo: usize) -> usize {
    0x0100 + (4 * fifo)
}
pub const DOTG_HCFG: usize = 0x0400;
pub const DOTG_HFIR: usize = 0x0404;
pub const DOTG_HFNUM: usize = 0x0408;
pub const DOTG_HPTXSTS: usize = 0x0410;
pub const DOTG_HAINT: usize = 0x0414;
pub const DOTG_HAINTMSK: usize = 0x0418;
pub const DOTG_HPRT: usize = 0x0440;
pub const fn DOTG_HCCHAR(ch: usize) -> usize {
    0x0500 + (32 * ch)
}
pub const fn DOTG_HCSPLT(ch: usize) -> usize {
    0x0504 + (32 * ch)
}
pub const fn DOTG_HCINT(ch: usize) -> usize {
    0x0508 + (32 * ch)
}
pub const fn DOTG_HCINTMSK(ch: usize) -> usize {
    0x050C + (32 * ch)
}
pub const fn DOTG_HCTSIZ(ch: usize) -> usize {
    0x0510 + (32 * ch)
}
pub const fn DOTG_HCDMA(ch: usize) -> usize {
    0x0514 + (32 * ch)
}
pub const fn DOTG_HCDMAI(ch: usize) -> usize {
    0x0514 + (32 * ch)
}
pub const fn DOTG_HCDMAO(ch: usize) -> usize {
    0x0514 + (32 * ch)
}
pub const fn DOTG_HCDMAB(ch: usize) -> usize {
    0x051C + (32 * ch)
}
// Device Mode
pub const DOTG_DCFG: usize = 0x0800;
pub const DOTG_DCTL: usize = 0x0804;
pub const DOTG_DSTS: usize = 0x0808;
pub const DOTG_DIEPMSK: usize = 0x0810;
pub const DOTG_DOEPMSK: usize = 0x0814;
pub const DOTG_DAINT: usize = 0x0818;
pub const DOTG_DAINTMSK: usize = 0x081C;
pub const DOTG_DTKNQR1: usize = 0x0820;
pub const DOTG_DTKNQR2: usize = 0x0824;
pub const DOTG_DVBUSDIS: usize = 0x0828;
pub const DOTG_DVBUSPULSE: usize = 0x082C;
pub const DOTG_DTHRCTL: usize = 0x0830;
pub const DOTG_DTKNQR4: usize = 0x0834;
pub const DOTG_DIEPEMPMSK: usize = 0x0834;
pub const DOTG_DEACHINT: usize = 0x0838;
pub const DOTG_DEACHINTMSK: usize = 0x083C;
pub const fn DOTG_DIEPEACHINTMSK(ch: u32) -> u32 {
    0x0840 + (4 * ch)
}
pub const fn DOTG_DOEPEACHINTMSK(ch: u32) -> u32 {
    0x0880 + (4 * ch)
}
pub const fn DOTG_DIEPCTL(ep: u32) -> u32 {
    0x0900 + (32 * ep)
}
pub const fn DOTG_DIEPINT(ep: u32) -> u32 {
    0x0908 + (32 * ep)
}
pub const fn DOTG_DIEPTSIZ(ep: u32) -> u32 {
    0x0910 + (32 * ep)
}
pub const fn DOTG_DIEPDMA(ep: u32) -> u32 {
    0x0914 + (32 * ep)
}
pub const fn DOTG_DTXFSTS(ep: u32) -> u32 {
    0x0918 + (32 * ep)
}
pub const fn DOTG_DIEPDMAB(ep: u32) -> u32 {
    0x091c + (32 * ep)
}
pub const fn DOTG_DOEPCTL(ep: u32) -> u32 {
    0x0B00 + (32 * ep)
}
pub const fn DOTG_DOEPFN(ep: u32) -> u32 {
    0x0B04 + (32 * ep)
}
pub const fn DOTG_DOEPINT(ep: u32) -> u32 {
    0x0B08 + (32 * ep)
}
pub const fn DOTG_DOEPTSIZ(ep: u32) -> u32 {
    0x0B10 + (32 * ep)
}
pub const fn DOTG_DOEPDMA(ep: u32) -> u32 {
    0x0B14 + (32 * ep)
}
pub const fn DOTG_DOEPDMAB(ep: u32) -> u32 {
    0x0B1c + (32 * ep)
}

// Register address
pub const DOTG_PCGCCTL: usize = 0x0E00;

// FIFO access registers (PIO-mode)
pub const DOTG_DFIFO: fn(n: u32) -> u32 = |n| 0x1000 + (0x1000 * n);

// GOTGCTL constants
pub const GOTGCTL_CHIRP_ON: u32 = 1 << 27;
pub const GOTGCTL_BSESVLD: u32 = 1 << 19;
pub const GOTGCTL_ASESVLD: u32 = 1 << 18;
pub const GOTGCTL_DBNCTIME: u32 = 1 << 17;
pub const GOTGCTL_CONIDSTS: u32 = 1 << 16;
pub const GOTGCTL_DEVHNPEN: u32 = 1 << 11;
pub const GOTGCTL_HSTSETHNPEN: u32 = 1 << 10;
pub const GOTGCTL_HNPREQ: u32 = 1 << 9;
pub const GOTGCTL_HSTNEGSCS: u32 = 1 << 8;
pub const GOTGCTL_SESREQ: u32 = 1 << 1;
pub const GOTGCTL_SESREQSCS: u32 = 1 << 0;
pub const GOTGCTL_DBNCEDONE: u32 = 1 << 19;
pub const GOTGCTL_ADEVTOUTCHG: u32 = 1 << 18;
pub const GOTGCTL_HSTNEGDET: u32 = 1 << 17;
pub const GOTGCTL_HSTNEGSUCSTSCHG: u32 = 1 << 9;
pub const GOTGCTL_SESREQSUCSTSCHG: u32 = 1 << 8;
pub const GOTGCTL_SESENDDET: u32 = 1 << 2;

// GAHBCFG constants
pub const GAHBCFG_PTXFEMPLVL: u32 = 1 << 8;
pub const GAHBCFG_NPTXFEMPLVL: u32 = 1 << 7;
pub const GAHBCFG_DMAEN: u32 = 1 << 5;
pub const GAHBCFG_HBSTLEN_MASK: u32 = 0x0000001e;
pub const GAHBCFG_HBSTLEN_SHIFT: u32 = 1;
pub const GAHBCFG_GLBLINTRMSK: u32 = 1 << 0;

// GUSBCFG constants
pub const GUSBCFG_CORRUPTTXPACKET: u32 = 1 << 31;
pub const GUSBCFG_FORCEDEVMODE: u32 = 1 << 30;
pub const GUSBCFG_FORCEHOSTMODE: u32 = 1 << 29;
pub const GUSBCFG_NO_PULLUP: u32 = 1 << 27;
pub const GUSBCFG_IC_USB_CAP: u32 = 1 << 26;
pub const GUSBCFG_TERMSELDLPULSE: u32 = 1 << 22;
pub const GUSBCFG_ULPIEXTVBUSINDICATOR: u32 = 1 << 21;
pub const GUSBCFG_ULPIEXTVBUSDRV: u32 = 1 << 20;
pub const GUSBCFG_ULPICLKSUSM: u32 = 1 << 19;
pub const GUSBCFG_ULPIAUTORES: u32 = 1 << 18;
pub const GUSBCFG_ULPIFSLS: u32 = 1 << 17;
pub const GUSBCFG_OTGI2CSEL: u32 = 1 << 16;
pub const GUSBCFG_PHYLPWRCLKSEL: u32 = 1 << 15;
pub const GUSBCFG_USBTRDTIM_MASK: u32 = 0x00003c00;
pub const GUSBCFG_USBTRDTIM_SHIFT: u32 = 10;
pub const GUSBCFG_TRD_TIM_SET: fn(x: u32) -> u32 = |x| ((x & 15) << 10);
pub const GUSBCFG_HNPCAP: u32 = 1 << 9;
pub const GUSBCFG_SRPCAP: u32 = 1 << 8;
pub const GUSBCFG_DDRSEL: u32 = 1 << 7;
pub const GUSBCFG_PHYSEL: u32 = 1 << 6;
pub const GUSBCFG_FSINTF: u32 = 1 << 5;
pub const GUSBCFG_ULPI_UTMI_SEL: u32 = 1 << 4;
pub const GUSBCFG_PHYIF: u32 = 1 << 3;
pub const GUSBCFG_TOUTCAL_MASK: u32 = 0x00000007;
pub const GUSBCFG_TOUTCAL_SHIFT: u32 = 0;

// STM32F4 constants
pub const DOTG_GGPIO_NOVBUSSENS: u32 = 1 << 21;
pub const DOTG_GGPIO_SOFOUTEN: u32 = 1 << 20;
pub const DOTG_GGPIO_VBUSBSEN: u32 = 1 << 19;
pub const DOTG_GGPIO_VBUSASEN: u32 = 1 << 18;
pub const DOTG_GGPIO_I2CPADEN: u32 = 1 << 17;
pub const DOTG_GGPIO_PWRDWN: u32 = 1 << 16;

// GRSTCTL constants
pub const GRSTCTL_AHBIDLE: u32 = 1 << 31;
pub const GRSTCTL_DMAREQ: u32 = 1 << 30;
pub const GRSTCTL_TXFNUM_MASK: u32 = 0x000007c0;
pub const GRSTCTL_TXFNUM_SHIFT: u32 = 6;
pub const GRSTCTL_TXFIFO: fn(n: u32) -> u32 = |n| ((n & 31) << 6);
pub const GRSTCTL_TXFFLSH: u32 = 1 << 5;
pub const GRSTCTL_RXFFLSH: u32 = 1 << 4;
pub const GRSTCTL_INTKNQFLSH: u32 = 1 << 3;
pub const GRSTCTL_FRMCNTRRST: u32 = 1 << 2;
pub const GRSTCTL_HSFTRST: u32 = 1 << 1;
pub const GRSTCTL_CSFTRST: u32 = 1 << 0;

// GINTSTS constants
pub const GINTSTS_WKUPINT: u32 = 1 << 31;
pub const GINTSTS_SESSREQINT: u32 = 1 << 30;
pub const GINTSTS_DISCONNINT: u32 = 1 << 29;
pub const GINTSTS_CONIDSTSCHNG: u32 = 1 << 28;
pub const GINTSTS_LPM: u32 = 1 << 27;
pub const GINTSTS_PTXFEMP: u32 = 1 << 26;
pub const GINTSTS_HCHINT: u32 = 1 << 25;
pub const GINTSTS_PRTINT: u32 = 1 << 24;
pub const GINTSTS_RESETDET: u32 = 1 << 23;
pub const GINTSTS_FETSUSP: u32 = 1 << 22;
pub const GINTSTS_INCOMPLP: u32 = 1 << 21;
pub const GINTSTS_INCOMPISOIN: u32 = 1 << 20;
pub const GINTSTS_OEPINT: u32 = 1 << 19;
pub const GINTSTS_IEPINT: u32 = 1 << 18;
pub const GINTSTS_EPMIS: u32 = 1 << 17;
pub const GINTSTS_RESTORE_DONE: u32 = 1 << 16;
pub const GINTSTS_EOPF: u32 = 1 << 15;
pub const GINTSTS_ISOOUTDROP: u32 = 1 << 14;
pub const GINTSTS_ENUMDONE: u32 = 1 << 13;
pub const GINTSTS_USBRST: u32 = 1 << 12;
pub const GINTSTS_USBSUSP: u32 = 1 << 11;
pub const GINTSTS_ERLYSUSP: u32 = 1 << 10;
pub const GINTSTS_I2CINT: u32 = 1 << 9;
pub const GINTSTS_ULPICKINT: u32 = 1 << 8;
pub const GINTSTS_GOUTNAKEFF: u32 = 1 << 7;
pub const GINTSTS_GINNAKEFF: u32 = 1 << 6;
pub const GINTSTS_NPTXFEMP: u32 = 1 << 5;
pub const GINTSTS_RXFLVL: u32 = 1 << 4;
pub const GINTSTS_SOF: u32 = 1 << 3;
pub const GINTSTS_OTGINT: u32 = 1 << 2;
pub const GINTSTS_MODEMIS: u32 = 1 << 1;
pub const GINTSTS_CURMOD: u32 = 1 << 0;

// GINTMSK definitions
pub const GINTMSK_WKUPINTMSK: u32 = 1 << 31;
pub const GINTMSK_SESSREQINTMSK: u32 = 1 << 30;
pub const GINTMSK_DISCONNINTMSK: u32 = 1 << 29;
pub const GINTMSK_CONIDSTSCHNGMSK: u32 = 1 << 28;
pub const GINTMSK_PTXFEMPMSK: u32 = 1 << 26;
pub const GINTMSK_HCHINTMSK: u32 = 1 << 25;
pub const GINTMSK_PRTINTMSK: u32 = 1 << 24;
pub const GINTMSK_FETSUSPMSK: u32 = 1 << 22;
pub const GINTMSK_INCOMPLPMSK: u32 = 1 << 21;
pub const GINTMSK_INCOMPISOINMSK: u32 = 1 << 20;
pub const GINTMSK_OEPINTMSK: u32 = 1 << 19;
pub const GINTMSK_IEPINTMSK: u32 = 1 << 18;
pub const GINTMSK_EPMISMSK: u32 = 1 << 17;
pub const GINTMSK_EOPFMSK: u32 = 1 << 15;
pub const GINTMSK_ISOOUTDROPMSK: u32 = 1 << 14;
pub const GINTMSK_ENUMDONEMSK: u32 = 1 << 13;
pub const GINTMSK_USBRSTMSK: u32 = 1 << 12;
pub const GINTMSK_USBSUSPMSK: u32 = 1 << 11;
pub const GINTMSK_ERLYSUSPMSK: u32 = 1 << 10;
pub const GINTMSK_I2CINTMSK: u32 = 1 << 9;
pub const GINTMSK_ULPICKINTMSK: u32 = 1 << 8;
pub const GINTMSK_GOUTNAKEFFMSK: u32 = 1 << 7;
pub const GINTMSK_GINNAKEFFMSK: u32 = 1 << 6;
pub const GINTMSK_NPTXFEMPMSK: u32 = 1 << 5;
pub const GINTMSK_RXFLVLMSK: u32 = 1 << 4;
pub const GINTMSK_SOFMSK: u32 = 1 << 3;
pub const GINTMSK_OTGINTMSK: u32 = 1 << 2;
pub const GINTMSK_MODEMISMSK: u32 = 1 << 1;
pub const GINTMSK_CURMODMSK: u32 = 1 << 0;

// GRXSTSRH definitions
pub const GRXSTSRH_PKTSTS_MASK: u32 = 0x001e0000;
pub const GRXSTSRH_PKTSTS_SHIFT: u32 = 17;
pub const GRXSTSRH_DPID_MASK: u32 = 0x00018000;
pub const GRXSTSRH_DPID_SHIFT: u32 = 15;
pub const GRXSTSRH_BCNT_MASK: u32 = 0x00007ff0;
pub const GRXSTSRH_BCNT_SHIFT: u32 = 4;
pub const GRXSTSRH_CHNUM_MASK: u32 = 0x0000000f;
pub const GRXSTSRH_CHNUM_SHIFT: u32 = 0;

// GRXSTSRD definitions
pub const GRXSTSRD_FN_MASK: u32 = 0x01e00000;
pub const GRXSTSRD_FN_SHIFT: u32 = 21;
pub const GRXSTSRD_PKTSTS_MASK: u32 = 0x001e0000;
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
pub const GRXSTSRD_BCNT_MASK: u32 = 0x00007ff0;
pub const GRXSTSRD_BCNT_SHIFT: u32 = 4;
pub const GRXSTSRD_CHNUM_MASK: u32 = 0x0000000f;
pub const GRXSTSRD_CHNUM_SHIFT: u32 = 0;

// GRXFSIZ definitions
pub const GRXFSIZ_RXFDEP_MASK: u32 = 0x0000ffff;
pub const GRXFSIZ_RXFDEP_SHIFT: u32 = 0;

// GNPTXFSIZ definitions
pub const GNPTXFSIZ_NPTXFDEP_MASK: u32 = 0xffff0000;
pub const GNPTXFSIZ_NPTXFDEP_SHIFT: u32 = 0;
pub const GNPTXFSIZ_NPTXFSTADDR_MASK: u32 = 0x0000ffff;
pub const GNPTXFSIZ_NPTXFSTADDR_SHIFT: u32 = 16;

// GNPTXSTS definitions
pub const GNPTXSTS_NPTXQTOP_SHIFT: u32 = 24;
pub const GNPTXSTS_NPTXQTOP_MASK: u32 = 0x7f000000;
pub const GNPTXSTS_NPTXQSPCAVAIL_SHIFT: u32 = 16;
pub const GNPTXSTS_NPTXQSPCAVAIL_MASK: u32 = 0x00ff0000;
pub const GNPTXSTS_NPTXFSPCAVAIL_SHIFT: u32 = 0;
pub const GNPTXSTS_NPTXFSPCAVAIL_MASK: u32 = 0x0000ffff;

// GI2CCTL definitions
pub const GI2CCTL_BSYDNE_SC: u32 = 1 << 31;
pub const GI2CCTL_RW: u32 = 1 << 30;
pub const GI2CCTL_I2CDATSE0: u32 = 1 << 28;
pub const GI2CCTL_I2CDEVADR_SHIFT: u32 = 26;
pub const GI2CCTL_I2CDEVADR_MASK: u32 = 0x0c000000;
pub const GI2CCTL_I2CSUSPCTL: u32 = 1 << 25;
pub const GI2CCTL_ACK: u32 = 1 << 24;
pub const GI2CCTL_I2CEN: u32 = 1 << 23;
pub const GI2CCTL_ADDR_SHIFT: u32 = 16;
pub const GI2CCTL_ADDR_MASK: u32 = 0x007f0000;
pub const GI2CCTL_REGADDR_SHIFT: u32 = 8;
pub const GI2CCTL_REGADDR_MASK: u32 = 0x0000ff00;
pub const GI2CCTL_RWDATA_SHIFT: u32 = 0;
pub const GI2CCTL_RWDATA_MASK: u32 = 0x000000ff;

// GPVNDCTL definitions
pub const GPVNDCTL_DISULPIDRVR: u32 = 1 << 31;
pub const GPVNDCTL_VSTSDONE: u32 = 1 << 27;
pub const GPVNDCTL_VSTSBSY: u32 = 1 << 26;
pub const GPVNDCTL_NEWREGREQ: u32 = 1 << 25;
pub const GPVNDCTL_REGWR: u32 = 1 << 22;
pub const GPVNDCTL_REGADDR_SHIFT: u32 = 16;
pub const GPVNDCTL_REGADDR_MASK: u32 = 0x003f0000;
pub const GPVNDCTL_VCTRL_SHIFT: u32 = 8;
pub const GPVNDCTL_VCTRL_MASK: u32 = 0x0000ff00;
pub const GPVNDCTL_REGDATA_SHIFT: u32 = 0;
pub const GPVNDCTL_REGDATA_MASK: u32 = 0x000000ff;

// GGPIO definitions
pub const GGPIO_GPO_SHIFT: u32 = 16;
pub const GGPIO_GPO_MASK: u32 = 0xffff0000;
pub const GGPIO_GPI_SHIFT: u32 = 0;
pub const GGPIO_GPI_MASK: u32 = 0x0000ffff;

// GHWCFG1 definitions
pub const GHWCFG1_BIDIR: u32 = 0;
pub const GHWCFG1_IN: u32 = 1;
pub const GHWCFG1_OUT: u32 = 2;

// GHWCFG2 definitions
pub const GHWCFG2_TKNQDEPTH_SHIFT: u32 = 26;
pub const GHWCFG2_TKNQDEPTH_MASK: u32 = 0x7c000000;
pub const GHWCFG2_PTXQDEPTH_SHIFT: u32 = 24;
pub const GHWCFG2_PTXQDEPTH_MASK: u32 = 0x03000000;
pub const GHWCFG2_NPTXQDEPTH_SHIFT: u32 = 22;
pub const GHWCFG2_NPTXQDEPTH_MASK: u32 = 0x00c00000;
pub const GHWCFG2_MPI: u32 = 1 << 20;
pub const GHWCFG2_DYNFIFOSIZING: u32 = 1 << 19;
pub const GHWCFG2_PERIOSUPPORT: u32 = 1 << 18;
pub const GHWCFG2_NUMHSTCHNL_SHIFT: u32 = 14;
pub const GHWCFG2_NUMHSTCHNL_MASK: u32 = 0x0003c000;
pub const GHWCFG2_NUMDEVEPS_SHIFT: u32 = 10;
pub const GHWCFG2_NUMDEVEPS_MASK: u32 = 0x00003c00;
pub const GHWCFG2_FSPHYTYPE_SHIFT: u32 = 8;
pub const GHWCFG2_FSPHYTYPE_MASK: u32 = 0x00000300;
pub const GHWCFG2_HSPHYTYPE_SHIFT: u32 = 6;
pub const GHWCFG2_HSPHYTYPE_MASK: u32 = 0x000000c0;
pub const GHWCFG2_SINGPNT: u32 = 1 << 5;
pub const GHWCFG2_OTGARCH_SHIFT: u32 = 3;
pub const GHWCFG2_OTGARCH_MASK: u32 = 0x00000018;
pub const GHWCFG2_OTGMODE_SHIFT: u32 = 0;
pub const GHWCFG2_OTGMODE_MASK: u32 = 0x00000007;

// GHWCFG3 definitions
pub const GHWCFG3_DFIFODEPTH_SHIFT: u32 = 16;
pub const GHWCFG3_DFIFODEPTH_MASK: u32 = 0xffff0000;
pub const GHWCFG3_RSTTYPE: u32 = 1 << 11;
pub const GHWCFG3_OPTFEATURE: u32 = 1 << 10;
pub const GHWCFG3_VNDCTLSUPT: u32 = 1 << 9;
pub const GHWCFG3_I2CINTSEL: u32 = 1 << 8;
pub const GHWCFG3_OTGEN: u32 = 1 << 7;
pub const GHWCFG3_PKTSIZEWIDTH_SHIFT: u32 = 4;
pub const GHWCFG3_PKTSIZEWIDTH_MASK: u32 = 0x00000070;
pub const GHWCFG3_XFERSIZEWIDTH_SHIFT: u32 = 0;
pub const GHWCFG3_XFERSIZEWIDTH_MASK: u32 = 0x0000000f;

// GHWCFG4 definitions
pub const GHWCFG4_SESSENDFLTR: u32 = 1 << 24;
pub const GHWCFG4_BVALIDFLTR: u32 = 1 << 23;
pub const GHWCFG4_AVALIDFLTR: u32 = 1 << 22;
pub const GHWCFG4_VBUSVALIDFLTR: u32 = 1 << 21;
pub const GHWCFG4_IDDGFLTR: u32 = 1 << 20;
pub const GHWCFG4_NUMCTLEPS_SHIFT: u32 = 16;
pub const GHWCFG4_NUMCTLEPS_MASK: u32 = 0x000f0000;
pub const GHWCFG4_PHYDATAWIDTH_SHIFT: u32 = 14;
pub const GHWCFG4_PHYDATAWIDTH_MASK: u32 = 0x0000c000;
pub const GHWCFG4_AHBFREQ: u32 = 1 << 5;
pub const GHWCFG4_ENABLEPWROPT: u32 = 1 << 4;
pub const GHWCFG4_NUMDEVPERIOEPS_SHIFT: u32 = 0;
pub const GHWCFG4_NUMDEVPERIOEPS_MASK: u32 = 0x0000000f;

pub const fn GHWCFG1_GET_DIR(x: u32, n: u32) -> u32 {
    (x >> (2 * n)) & 3
}
pub const fn GRXSTSRD_FN_GET(x: u32) -> u32 {
    ((x) >> 21) & 15
}
pub const fn GRXSTSRD_BCNT_GET(x: u32) -> u32 {
    ((x) >> 4) & 0x7FF
}
pub const fn GRXSTSRD_CHNUM_GET(x: u32) -> u32 {
    (x) & 15
}
pub const fn GHWCFG2_NUMHSTCHNL_GET(x: u32) -> u32 {
    (((x) >> 14) & 15) + 1
}
pub const fn GHWCFG2_NUMDEVEPS_GET(x: u32) -> u32 {
    (((x) >> 10) & 15) + 1
}
pub const fn GHWCFG3_DFIFODEPTH_GET(x: u32) -> u32 {
    (x) >> 16
}
pub const fn GHWCFG3_PKTSIZE_GET(x: u32) -> u32 {
    0x10 << (((x) >> 4) & 7)
}
pub const fn GHWCFG3_XFRRSIZE_GET(x: u32) -> u32 {
    0x400 << (((x) >> 0) & 15)
}
pub const fn GHWCFG4_NUM_IN_EP_GET(x: u32) -> u32 {
    (((x) >> 26) & 15) + 1
}
pub const fn GHWCFG4_NUMCTLEPS_GET(x: u32) -> u32 {
    ((x) >> 16) & 15
}
pub const fn GHWCFG4_NUMDEVPERIOEPS_GET(x: u32) -> u32 {
    ((x) >> 0) & 15
}

// GLPMCFG
pub const GLPMCFG_HSIC_CONN: u32 = 1 << 30;

// GPWRDN
pub const GPWRDN_BVALID: u32 = 1 << 22;
pub const GPWRDN_IDDIG: u32 = 1 << 21;
pub const GPWRDN_CONNDET_INT: u32 = 1 << 14;
pub const GPWRDN_CONNDET: u32 = 1 << 13;
pub const GPWRDN_DISCONN_INT: u32 = 1 << 12;
pub const GPWRDN_DISCONN: u32 = 1 << 11;
pub const GPWRDN_RESETDET_INT: u32 = 1 << 10;
pub const GPWRDN_RESETDET: u32 = 1 << 9;
pub const GPWRDN_LINESTATE_INT: u32 = 1 << 8;
pub const GPWRDN_LINESTATE: u32 = 1 << 7;
pub const GPWRDN_DISABLE_VBUS: u32 = 1 << 6;
pub const GPWRDN_POWER_DOWN: u32 = 1 << 5;
pub const GPWRDN_POWER_DOWN_RST: u32 = 1 << 4;
pub const GPWRDN_POWER_DOWN_CLAMP: u32 = 1 << 3;
pub const GPWRDN_RESTORE: u32 = 1 << 2;
pub const GPWRDN_PMU_ACTIVE: u32 = 1 << 1;
pub const GPWRDN_PMU_IRQ_SEL: u32 = 1 << 0;

// HPTXFSIZ
pub const HPTXFSIZ_PTXFSIZE_SHIFT: u32 = 16;
pub const HPTXFSIZ_PTXFSIZE_MASK: u32 = 0xffff0000;
pub const HPTXFSIZ_PTXFSTADDR_SHIFT: u32 = 0;
pub const HPTXFSIZ_PTXFSTADDR_MASK: u32 = 0x0000ffff;

// DPTXFSIZN
pub const DPTXFSIZN_DPTXFSIZE_SHIFT: u32 = 16;
pub const DPTXFSIZN_DPTXFSIZE_MASK: u32 = 0xffff0000;
pub const DPTXFSIZN_PTXFSTADDR_SHIFT: u32 = 0;
pub const DPTXFSIZN_PTXFSTADDR_MASK: u32 = 0x0000ffff;

// DIEPTXFN
pub const DIEPTXFN_INEPNTXFDEP_SHIFT: u32 = 16;
pub const DIEPTXFN_INEPNTXFDEP_MASK: u32 = 0xffff0000;
pub const DIEPTXFN_INEPNTXFSTADDR_SHIFT: u32 = 0;
pub const DIEPTXFN_INEPNTXFSTADDR_MASK: u32 = 0x0000ffff;

// HCFG
pub const HCFG_MODECHANGERDY: u32 = 1 << 31;
pub const HCFG_PERSCHEDENABLE: u32 = 1 << 26;
pub const HCFG_FLENTRIES_SHIFT: u32 = 24;
pub const HCFG_FLENTRIES_MASK: u32 = 0x03000000;
pub const HCFG_FLENTRIES_8: u32 = 0;
pub const HCFG_FLENTRIES_16: u32 = 1;
pub const HCFG_FLENTRIES_32: u32 = 2;
pub const HCFG_FLENTRIES_64: u32 = 3;
pub const HCFG_MULTISEGDMA: u32 = 1 << 23;
pub const HCFG_32KHZSUSPEND: u32 = 1 << 7;
pub const HCFG_FSLSSUPP: u32 = 1 << 2;
pub const HCFG_FSLSPCLKSEL_SHIFT: u32 = 0;
pub const HCFG_FSLSPCLKSEL_MASK: u32 = 0x00000003;

// HFIR
pub const HFIR_RELOADCTRL: u32 = 1 << 16;
pub const HFIR_FRINT_SHIFT: u32 = 0;
pub const HFIR_FRINT_MASK: u32 = 0x0000ffff;

// HFNUM
pub const HFNUM_FRREM_SHIFT: u32 = 16;
pub const HFNUM_FRREM_MASK: u32 = 0xffff0000;
pub const HFNUM_FRNUM_SHIFT: u32 = 0;
pub const HFNUM_FRNUM_MASK: u32 = 0x0000ffff;

// HPTXSTS
pub const HPTXSTS_ODD: u32 = 1 << 31;
pub const HPTXSTS_CHAN_SHIFT: u32 = 27;
pub const HPTXSTS_CHAN_MASK: u32 = 0x78000000;
pub const HPTXSTS_TOKEN_SHIFT: u32 = 25;
pub const HPTXSTS_TOKEN_MASK: u32 = 0x06000000;
pub const HPTXSTS_TOKEN_ZL: u32 = 0;
pub const HPTXSTS_TOKEN_PING: u32 = 1;
pub const HPTXSTS_TOKEN_DISABLE: u32 = 2;
pub const HPTXSTS_TERMINATE: u32 = 1 << 24;
pub const HPTXSTS_PTXQSPCAVAIL_SHIFT: u32 = 16;
pub const HPTXSTS_PTXQSPCAVAIL_MASK: u32 = 0x00ff0000;
pub const HPTXSTS_PTXFSPCAVAIL_SHIFT: u32 = 0;
pub const HPTXSTS_PTXFSPCAVAIL_MASK: u32 = 0x0000ffff;

// HAINT
pub const HAINT_HAINT_SHIFT: u32 = 0;
pub const HAINT_HAINT_MASK: u32 = 0x0000ffff;
pub const HAINTMSK_HAINTMSK_SHIFT: u32 = 0;
pub const HAINTMSK_HAINTMSK_MASK: u32 = 0x0000ffff;

// HPRT
pub const HPRT_PRTSPD_SHIFT: u32 = 17;
pub const HPRT_PRTSPD_MASK: u32 = 0x00060000;
pub const HPRT_PRTSPD_HIGH: u32 = 0;
pub const HPRT_PRTSPD_FULL: u32 = 1;
pub const HPRT_PRTSPD_LOW: u32 = 2;
pub const HPRT_PRTTSTCTL_SHIFT: u32 = 13;
pub const HPRT_PRTTSTCTL_MASK: u32 = 0x0001e000;
pub const HPRT_PRTPWR: u32 = 1 << 12;
pub const HPRT_PRTLNSTS_SHIFT: u32 = 10;
pub const HPRT_PRTLNSTS_MASK: u32 = 0x00000c00;
pub const HPRT_PRTRST: u32 = 1 << 8;
pub const HPRT_PRTSUSP: u32 = 1 << 7;
pub const HPRT_PRTRES: u32 = 1 << 6;
pub const HPRT_PRTOVRCURRCHNG: u32 = 1 << 5;
pub const HPRT_PRTOVRCURRACT: u32 = 1 << 4;
pub const HPRT_PRTENCHNG: u32 = 1 << 3;
pub const HPRT_PRTENA: u32 = 1 << 2;
pub const HPRT_PRTCONNDET: u32 = 1 << 1;
pub const HPRT_PRTCONNSTS: u32 = 1 << 0;

// HCCHAR
pub const HCCHAR_CHENA: u32 = 1 << 31;
pub const HCCHAR_CHDIS: u32 = 1 << 30;
pub const HCCHAR_ODDFRM: u32 = 1 << 29;
pub const HCCHAR_DEVADDR_SHIFT: u32 = 22;
pub const HCCHAR_DEVADDR_MASK: u32 = 0x1fc00000;
pub const HCCHAR_MC_SHIFT: u32 = 20;
pub const HCCHAR_MC_MASK: u32 = 0x00300000;
pub const HCCHAR_EPTYPE_SHIFT: u32 = 18;
pub const HCCHAR_EPTYPE_MASK: u32 = 0x000c0000;
pub const HCCHAR_LSPDDEV: u32 = 1 << 17;
pub const HCCHAR_EPDIR: u32 = 1 << 15;
pub const HCCHAR_EPDIR_IN: u32 = 1 << 15;
pub const HCCHAR_EPDIR_OUT: u32 = 0;
pub const HCCHAR_EPNUM_SHIFT: u32 = 11;
pub const HCCHAR_EPNUM_MASK: u32 = 0x00007800;
pub const HCCHAR_MPS_SHIFT: u32 = 0;
pub const HCCHAR_MPS_MASK: u32 = 0x000007ff;

// HCSPLT
pub const HCSPLT_SPLTENA: u32 = 1 << 31;
pub const HCSPLT_COMPSPLT: u32 = 1 << 16;
pub const HCSPLT_XACTPOS_SHIFT: u32 = 14;
pub const HCSPLT_XACTPOS_MASK: u32 = 0x0000c000;
pub const HCSPLT_XACTPOS_MIDDLE: u32 = 0;
pub const HCSPLT_XACTPOS_LAST: u32 = 1;
pub const HCSPLT_XACTPOS_BEGIN: u32 = 2;
pub const HCSPLT_XACTPOS_ALL: u32 = 3;
pub const HCSPLT_XACTLEN_BURST: u32 = 1023; // bytes
pub const HCSPLT_HUBADDR_SHIFT: u32 = 7;
pub const HCSPLT_HUBADDR_MASK: u32 = 0x00003f80;
pub const HCSPLT_PRTADDR_SHIFT: u32 = 0;
pub const HCSPLT_PRTADDR_MASK: u32 = 0x0000007f;

// HCINT
pub const HCINT_ERRORS: u32 = HCINT_BBLERR | HCINT_XACTERR;
pub const HCINT_RETRY: u32 = HCINT_DATATGLERR | HCINT_FRMOVRUN | HCINT_NAK;
pub const HCINT_DEFAULT_MASK: u32 = HCINT_STALL
    | HCINT_BBLERR
    | HCINT_XACTERR
    | HCINT_NAK
    | HCINT_ACK
    | HCINT_NYET
    | HCINT_CHHLTD
    | HCINT_FRMOVRUN
    | HCINT_DATATGLERR;
pub const HCINT_HCH_DONE_MASK: u32 =
    HCINT_ACK | HCINT_RETRY | HCINT_NYET | HCINT_ERRORS | HCINT_STALL | HCINT_SOFTWARE_ONLY;

pub const HCINT_SOFTWARE_ONLY: u32 = 1 << 20; // BSD only
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

// HCINTMSK constants
pub const HCINTMSK_DATATGLERRMSK: u32 = 1 << 10;
pub const HCINTMSK_FRMOVRUNMSK: u32 = 1 << 9;
pub const HCINTMSK_BBLERRMSK: u32 = 1 << 8;
pub const HCINTMSK_XACTERRMSK: u32 = 1 << 7;
pub const HCINTMSK_NYETMSK: u32 = 1 << 6;
pub const HCINTMSK_ACKMSK: u32 = 1 << 5;
pub const HCINTMSK_NAKMSK: u32 = 1 << 4;
pub const HCINTMSK_STALLMSK: u32 = 1 << 3;
pub const HCINTMSK_AHBERRMSK: u32 = 1 << 2;
pub const HCINTMSK_CHHLTDMSK: u32 = 1 << 1;
pub const HCINTMSK_XFERCOMPLMSK: u32 = 1 << 0;

// HCTSIZ constants
pub const HCTSIZ_DOPNG: u32 = 1 << 31;
pub const HCTSIZ_PID_SHIFT: u32 = 29;
pub const HCTSIZ_PID_MASK: u32 = 0x60000000;
pub const HCTSIZ_PID_DATA0: u32 = 0;
pub const HCTSIZ_PID_DATA2: u32 = 1;
pub const HCTSIZ_PID_DATA1: u32 = 2;
pub const HCTSIZ_PID_MDATA: u32 = 3;
pub const HCTSIZ_PID_SETUP: u32 = 3;
pub const HCTSIZ_PKTCNT_SHIFT: u32 = 19;
pub const HCTSIZ_PKTCNT_MASK: u32 = 0x1ff80000;
pub const HCTSIZ_XFERSIZE_SHIFT: u32 = 0;
pub const HCTSIZ_XFERSIZE_MASK: u32 = 0x0007ffff;

// DCFG constants
pub const DCFG_EPMISCNT_SHIFT: u32 = 18;
pub const DCFG_EPMISCNT_MASK: u32 = 0x007c0000;
pub const DCFG_PERFRINT_SHIFT: u32 = 11;
pub const DCFG_PERFRINT_MASK: u32 = 0x00001800;
pub const DCFG_DEVADDR_SHIFT: u32 = 4;
pub const DCFG_DEVADDR_MASK: u32 = 0x000007f0;
pub const DCFG_NZSTSOUTHSHK: u32 = 1 << 2;
pub const DCFG_DEVSPD_SHIFT: u32 = 0;
pub const DCFG_DEVSPD_MASK: u32 = 0x00000003;
pub const DCFG_DEVSPD_HI: u32 = 0;
pub const DCFG_DEVSPD_FULL20: u32 = 1;
pub const DCFG_DEVSPD_FULL10: u32 = 3;

// Function-like macros converted to const fn
#[inline]
pub const fn DCFG_DEVADDR_SET(x: u32) -> u32 {
    ((x) & 0x7F) << 4
}

#[inline]
pub const fn DCFG_DEVSPD_SET(x: u32) -> u32 {
    (x) & 0x3
}

// DCTL constants
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

// DSTS constants
pub const DSTS_SOFFN_SHIFT: u32 = 8;
pub const DSTS_SOFFN_MASK: u32 = 0x003fff00;
pub const DSTS_ERRTICERR: u32 = 1 << 3;
pub const DSTS_ENUMSPD_SHIFT: u32 = 1;
pub const DSTS_ENUMSPD_MASK: u32 = 0x00000006;
pub const DSTS_ENUMSPD_HI: u32 = 0;
pub const DSTS_ENUMSPD_FULL20: u32 = 1;
pub const DSTS_ENUMSPD_LOW10: u32 = 2;
pub const DSTS_ENUMSPD_FULL10: u32 = 3;
pub const DSTS_SUSPSTS: u32 = 1 << 0;

pub const fn DSTS_SOFFN_GET(x: u32) -> u32 {
    ((x) >> 8) & 0x3FFF
}
pub const fn DSTS_ENUMSPD_GET(x: u32) -> u32 {
    ((x) >> 1) & 3
}

// DIEPMSK constants
pub const DIEPMSK_TXFIFOUNDRNMSK: u32 = 1 << 8;
pub const DIEPMSK_INEPNAKEFFMSK: u32 = 1 << 6;
pub const DIEPMSK_INTKNEPMISMSK: u32 = 1 << 5;
pub const DIEPMSK_INTKNTXFEMPMSK: u32 = 1 << 4;
pub const DIEPMSK_FIFOEMPTY: u32 = 1 << 4;
pub const DIEPMSK_TIMEOUTMSK: u32 = 1 << 3;
pub const DIEPMSK_AHBERRMSK: u32 = 1 << 2;
pub const DIEPMSK_EPDISBLDMSK: u32 = 1 << 1;
pub const DIEPMSK_XFERCOMPLMSK: u32 = 1 << 0;

// DOEPMSK constants
pub const DOEPMSK_OUTPKTERRMSK: u32 = 1 << 8;
pub const DOEPMSK_BACK2BACKSETUP: u32 = 1 << 6;
pub const DOEPMSK_OUTTKNEPDISMSK: u32 = 1 << 4;
pub const DOEPMSK_FIFOEMPTY: u32 = 1 << 4;
pub const DOEPMSK_SETUPMSK: u32 = 1 << 3;
pub const DOEPMSK_AHBERRMSK: u32 = 1 << 2;
pub const DOEPMSK_EPDISBLDMSK: u32 = 1 << 1;
pub const DOEPMSK_XFERCOMPLMSK: u32 = 1 << 0;

// DIEPINT constants
pub const DIEPINT_TXFIFOUNDRN: u32 = 1 << 8;
pub const DIEPINT_INEPNAKEFF: u32 = 1 << 6;
pub const DIEPINT_INTKNEPMIS: u32 = 1 << 5;
pub const DIEPINT_INTKNTXFEMP: u32 = 1 << 4;
pub const DIEPINT_TIMEOUT: u32 = 1 << 3;
pub const DIEPINT_AHBERR: u32 = 1 << 2;
pub const DIEPINT_EPDISBLD: u32 = 1 << 1;
pub const DIEPINT_XFERCOMPL: u32 = 1 << 0;

// DOEPINT constants
pub const DOEPINT_OUTPKTERR: u32 = 1 << 8;
pub const DOEPINT_BACK2BACKSETUP: u32 = 1 << 6;
pub const DOEPINT_OUTTKNEPDIS: u32 = 1 << 4;
pub const DOEPINT_SETUP: u32 = 1 << 3;
pub const DOEPINT_AHBERR: u32 = 1 << 2;
pub const DOEPINT_EPDISBLD: u32 = 1 << 1;
pub const DOEPINT_XFERCOMPL: u32 = 1 << 0;

// DAINT constants
pub const DAINT_INEPINT_MASK: u32 = 0xffff0000;
pub const DAINT_INEPINT_SHIFT: u32 = 0;
pub const DAINT_OUTEPINT_MASK: u32 = 0x0000ffff;
pub const DAINT_OUTEPINT_SHIFT: u32 = 16;

// DAINTMSK constants
pub const DAINTMSK_INEPINT_MASK: u32 = 0xffff0000;
pub const DAINTMSK_INEPINT_SHIFT: u32 = 0;
pub const DAINTMSK_OUTEPINT_MASK: u32 = 0x0000ffff;
pub const DAINTMSK_OUTEPINT_SHIFT: u32 = 16;

// DTKNQR1 constants
pub const DTKNQR1_EPTKN_SHIFT: u32 = 8;
pub const DTKNQR1_EPTKN_MASK: u32 = 0xffffff00;
pub const DTKNQR1_WRAPBIT: u32 = 1 << 7;
pub const DTKNQR1_INTKNWPTR_SHIFT: u32 = 0;
pub const DTKNQR1_INTKNWPTR_MASK: u32 = 0x0000001f;

// DVBUSDIS constants
pub const DVBUSDIS_DVBUSDIS_SHIFT: u32 = 0;
pub const DVBUSDIS_DVBUSDIS_MASK: u32 = 0x0000ffff;

// DVBUSPULSE constants
pub const DVBUSPULSE_DVBUSPULSE_SHIFT: u32 = 0;
pub const DVBUSPULSE_DVBUSPULSE_MASK: u32 = 0x00000fff;

// DTHRCTL constants
pub const DTHRCTL_ARBPRKEN: u32 = 1 << 27;
pub const DTHRCTL_RXTHRLEN_SHIFT: u32 = 17;
pub const DTHRCTL_RXTHRLEN_MASK: u32 = 0x03fe0000;
pub const DTHRCTL_RXTHREN: u32 = 1 << 16;
pub const DTHRCTL_TXTHRLEN_SHIFT: u32 = 2;
pub const DTHRCTL_TXTHRLEN_MASK: u32 = 0x000007fc;
pub const DTHRCTL_ISOTHREN: u32 = 1 << 1;
pub const DTHRCTL_NONISOTHREN: u32 = 1 << 0;

// DIEPEMPMSK constants
pub const DIEPEMPMSK_INEPTXFEMPMSK_SHIFT: u32 = 0;
pub const DIEPEMPMSK_INEPTXFEMPMSK_MASK: u32 = 0x0000ffff;

// DIEPCTL constants
pub const DIEPCTL_EPENA: u32 = 1 << 31;
pub const DIEPCTL_EPDIS: u32 = 1 << 30;
pub const DIEPCTL_SETD1PID: u32 = 1 << 29;
pub const DIEPCTL_SETD0PID: u32 = 1 << 28;
pub const DIEPCTL_SNAK: u32 = 1 << 27;
pub const DIEPCTL_CNAK: u32 = 1 << 26;
pub const DIEPCTL_TXFNUM_SHIFT: u32 = 22;
pub const DIEPCTL_TXFNUM_MASK: u32 = 0x03c00000;
pub const DIEPCTL_STALL: u32 = 1 << 21;
pub const DIEPCTL_EPTYPE_SHIFT: u32 = 18;
pub const DIEPCTL_EPTYPE_MASK: u32 = 0x000c0000;
pub const DIEPCTL_EPTYPE_CONTROL: u32 = 0;
pub const DIEPCTL_EPTYPE_ISOC: u32 = 1;
pub const DIEPCTL_EPTYPE_BULK: u32 = 2;
pub const DIEPCTL_EPTYPE_INTERRUPT: u32 = 3;
pub const DIEPCTL_NAKSTS: u32 = 1 << 17;
pub const DIEPCTL_USBACTEP: u32 = 1 << 15;
pub const DIEPCTL_NEXTEP_SHIFT: u32 = 11;
pub const DIEPCTL_NEXTEP_MASK: u32 = 0x00007800;
pub const DIEPCTL_MPS_SHIFT: u32 = 0;
pub const DIEPCTL_MPS_MASK: u32 = 0x000007ff;
pub const DIEPCTL_MPS_64: u32 = 0 << 0;
pub const DIEPCTL_MPS_32: u32 = 1 << 0;
pub const DIEPCTL_MPS_16: u32 = 2 << 0;
pub const DIEPCTL_MPS_8: u32 = 3 << 0;

pub const fn DIEPCTL_TXFNUM_SET(n: u32) -> u32 {
    ((n) & 15) << 22
}
pub const fn DIEPCTL_EPTYPE_SET(n: u32) -> u32 {
    ((n) & 3) << 18
}
pub const fn DIEPCTL_MPS_SET(n: u32) -> u32 {
    (n) & 0x7FF
}

pub const DOEPCTL_EPENA: u32 = 1 << 31;
pub const DOEPCTL_EPDIS: u32 = 1 << 30;
pub const DOEPCTL_SETD1PID: u32 = 1 << 29;
pub const DOEPCTL_SETD0PID: u32 = 1 << 28;
pub const DOEPCTL_SNAK: u32 = 1 << 27;
pub const DOEPCTL_CNAK: u32 = 1 << 26;
pub const DOEPCTL_STALL: u32 = 1 << 21;
pub const DOEPCTL_EPTYPE_SHIFT: u32 = 18;
pub const DOEPCTL_EPTYPE_MASK: u32 = 0x000c0000;
pub const DOEPCTL_NAKSTS: u32 = 1 << 17;
pub const DOEPCTL_USBACTEP: u32 = 1 << 15;
pub const DOEPCTL_MPS_SHIFT: u32 = 0;
pub const DOEPCTL_MPS_MASK: u32 = 0x000007ff;
pub const DOEPCTL_MPS_64: u32 = 0 << 0;
pub const DOEPCTL_MPS_32: u32 = 1 << 0;
pub const DOEPCTL_MPS_16: u32 = 2 << 0;
pub const DOEPCTL_MPS_8: u32 = 3 << 0;
/* common bits */
pub const DXEPINT_TXFEMP: u32 = 1 << 7;
pub const DXEPINT_SETUP: u32 = 1 << 3;
pub const DXEPINT_XFER_COMPL: u32 = 1 << 0;
pub const DIEPTSIZ_XFERSIZE_MASK: u32 = 0x0007ffff;
pub const DIEPTSIZ_XFERSIZE_SHIFT: u32 = 0;
pub const DIEPTSIZ_PKTCNT_MASK: u32 = 0x1ff80000;
pub const DIEPTSIZ_PKTCNT_SHIFT: u32 = 19;
pub const DIEPTSIZ_MC_MASK: u32 = 0x60000000;
pub const DIEPTSIZ_MC_SHIFT: u32 = 29;
pub const DOEPTSIZ_XFERSIZE_MASK: u32 = 0x0007ffff;
pub const DOEPTSIZ_XFERSIZE_SHIFT: u32 = 0;
pub const DOEPTSIZ_PKTCNT_MASK: u32 = 0x1ff80000;
pub const DOEPTSIZ_PKTCNT_SHIFT: u32 = 19;
pub const DOEPTSIZ_MC_MASK: u32 = 0x60000000;
pub const DOEPTSIZ_MC_SHIFT: u32 = 29;
/* common bits */

pub const fn DOEPCTL_FNUM_SET(n: u32) -> u32 {
    ((n) & 15) << 22
}
pub const fn DOEPCTL_EPTYPE_SET(n: u32) -> u32 {
    ((n) & 3) << 18
}
pub const fn DOEPCTL_MPS_SET(n: u32) -> u32 {
    (n) & 0x7FF
}
pub const fn DXEPTSIZ_SET_MULTI(n: u32) -> u32 {
    ((n) & 3) << 29
}
pub const fn DXEPTSIZ_SET_NPKT(n: u32) -> u32 {
    ((n) & 0x3FF) << 19
}
pub const fn DXEPTSIZ_GET_NPKT(n: u32) -> u32 {
    ((n) >> 19) & 0x3FF
}
pub const fn DXEPTSIZ_SET_NBYTES(n: u32) -> u32 {
    ((n) & 0x7FFFFF) << 0
}
pub const fn DXEPTSIZ_GET_NBYTES(n: u32) -> u32 {
    ((n) >> 0) & 0x7FFFFF
}

// #[inline(always)]
// pub const fn ENDPOINT_MASK(x: u32, r#in: bool) -> u32 {
//     if r#in {
//         1u32 << ((x) & 15u32)
//     } else {
//         0x10000u32 << ((x) & 15u32)
//     }
// }
