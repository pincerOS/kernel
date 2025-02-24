/*-
 * SPDX-License-Identifier: BSD-2-Clause
 *
 * Copyright (c) 2008 Hans Petter Selasky. All rights reserved.
 * Copyright (c) 1998 The NetBSD Foundation, Inc. All rights reserved.
 * Copyright (c) 1998 Lennart Augustsson. All rights reserved.
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



pub const USB_FS_ISOC_UFRAME_MAX: u32 = 4; // exclusive unit
pub const USB_BUS_MAX: u32 = 256; // units
pub const USB_MAX_DEVICES: u32 = 128; // units
pub const USB_CONFIG_MAX: u32 = 65535; // bytes
pub const USB_IFACE_MAX: u32 = 32; // units
pub const USB_FIFO_MAX: u32 = 128; // units
pub const USB_MAX_EP_STREAMS: u32 = 8; // units
pub const USB_MAX_EP_UNITS: u32 = 32; // units
pub const USB_MAX_PORTS: u32 = 255; // units

pub const USB_MAX_FS_ISOC_FRAMES_PER_XFER: u32 = 120; // units
pub const USB_MAX_HS_ISOC_FRAMES_PER_XFER: u32 = 8 * 120; // units

pub const USB_HUB_MAX_DEPTH: u32 = 5;
pub const USB_EP0_BUFSIZE: u32 = 1024; // bytes
pub const USB_CS_RESET_LIMIT: u32 = 20; // failures = 20 * 50 ms = 1sec


/* Device status flags */
pub const UDS_SELF_POWERED: u16 = 0x0001;
pub const UDS_REMOTE_WAKEUP: u16 = 0x0002;

pub const UT_WRITE: u8 = 0x00;
pub const UT_READ: u8 = 0x80;
pub const UT_STANDARD: u8 = 0x00;
pub const UT_CLASS: u8 = 0x20;
pub const UT_VENDOR: u8 = 0x40;
pub const UT_DEVICE: u8 = 0x00;
pub const UT_INTERFACE: u8 = 0x01;
pub const UT_ENDPOINT: u8 = 0x02;
pub const UT_OTHER: u8 = 0x03;

pub const UT_READ_DEVICE: u8 = UT_READ | UT_STANDARD | UT_DEVICE;
pub const UT_READ_INTERFACE: u8 = UT_READ | UT_STANDARD | UT_INTERFACE;
pub const UT_READ_ENDPOINT: u8 = UT_READ | UT_STANDARD | UT_ENDPOINT;
pub const UT_WRITE_DEVICE: u8 = UT_WRITE | UT_STANDARD | UT_DEVICE;
pub const UT_WRITE_INTERFACE: u8 = UT_WRITE | UT_STANDARD | UT_INTERFACE;
pub const UT_WRITE_ENDPOINT: u8 = UT_WRITE | UT_STANDARD | UT_ENDPOINT;
pub const UT_READ_CLASS_DEVICE: u8 = UT_READ | UT_CLASS | UT_DEVICE;
pub const UT_READ_CLASS_INTERFACE: u8 = UT_READ | UT_CLASS | UT_INTERFACE;
pub const UT_READ_CLASS_OTHER: u8 = UT_READ | UT_CLASS | UT_OTHER;
pub const UT_READ_CLASS_ENDPOINT: u8 = UT_READ | UT_CLASS | UT_ENDPOINT;
pub const UT_WRITE_CLASS_DEVICE: u8 = UT_WRITE | UT_CLASS | UT_DEVICE;
pub const UT_WRITE_CLASS_INTERFACE: u8 = UT_WRITE | UT_CLASS | UT_INTERFACE;
pub const UT_WRITE_CLASS_OTHER: u8 = UT_WRITE | UT_CLASS | UT_OTHER;
pub const UT_WRITE_CLASS_ENDPOINT: u8 = UT_WRITE | UT_CLASS | UT_ENDPOINT;
pub const UT_READ_VENDOR_DEVICE: u8 = UT_READ | UT_VENDOR | UT_DEVICE;
pub const UT_READ_VENDOR_INTERFACE: u8 = UT_READ | UT_VENDOR | UT_INTERFACE;
pub const UT_READ_VENDOR_OTHER: u8 = UT_READ | UT_VENDOR | UT_OTHER;
pub const UT_READ_VENDOR_ENDPOINT: u8 = UT_READ | UT_VENDOR | UT_ENDPOINT;
pub const UT_WRITE_VENDOR_DEVICE: u8 = UT_WRITE | UT_VENDOR | UT_DEVICE;
pub const UT_WRITE_VENDOR_INTERFACE: u8 = UT_WRITE | UT_VENDOR | UT_INTERFACE;
pub const UT_WRITE_VENDOR_OTHER: u8 = UT_WRITE | UT_VENDOR | UT_OTHER;
pub const UT_WRITE_VENDOR_ENDPOINT: u8 = UT_WRITE | UT_VENDOR | UT_ENDPOINT;


pub const UPS_CURRENT_CONNECT_STATUS: u16 = 0x0001;
pub const UPS_PORT_ENABLED: u16 = 0x0002;
pub const UPS_SUSPEND: u16 = 0x0004;
pub const UPS_OVERCURRENT_INDICATOR: u16 = 0x0008;
pub const UPS_RESET: u16 = 0x0010;
pub const UPS_PORT_L1: u16 = 0x0020; // USB 2.0 only
// The link-state bits are valid for Super-Speed USB HUBs
pub const fn UPS_PORT_LINK_STATE_GET(x: u16) -> u16 {
    ((x >> 5) & 0xF)
}

pub const fn UPS_PORT_LINK_STATE_SET(x: u16) -> u16 {
    ((x & 0xF) << 5)
}
pub const UPS_PORT_LS_U0: u16 = 0x00;
pub const UPS_PORT_LS_U1: u16 = 0x01;
pub const UPS_PORT_LS_U2: u16 = 0x02;
pub const UPS_PORT_LS_U3: u16 = 0x03;
pub const UPS_PORT_LS_SS_DIS: u16 = 0x04;
pub const UPS_PORT_LS_RX_DET: u16 = 0x05;
pub const UPS_PORT_LS_SS_INA: u16 = 0x06;
pub const UPS_PORT_LS_POLL: u16 = 0x07;
pub const UPS_PORT_LS_RECOVER: u16 = 0x08;
pub const UPS_PORT_LS_HOT_RST: u16 = 0x09;
pub const UPS_PORT_LS_COMP_MODE: u16 = 0x0A;
pub const UPS_PORT_LS_LOOPBACK: u16 = 0x0B;
pub const UPS_PORT_LS_RESUME: u16 = 0x0F;
pub const UPS_PORT_POWER: u16 = 0x0100;
pub const UPS_PORT_POWER_SS: u16 = 0x0200; // super-speed only
pub const UPS_LOW_SPEED: u16 = 0x0200;
pub const UPS_HIGH_SPEED: u16 = 0x0400;
pub const UPS_OTHER_SPEED: u16 = 0x0600; // currently FreeBSD specific
pub const UPS_PORT_TEST: u16 = 0x0800;
pub const UPS_PORT_INDICATOR: u16 = 0x1000;
pub const UPS_PORT_MODE_DEVICE: u16 = 0x8000; // currently FreeBSD specific
pub const UPS_C_CONNECT_STATUS: u16 = 0x0001;
pub const UPS_C_PORT_ENABLED: u16 = 0x0002;
pub const UPS_C_SUSPEND: u16 = 0x0004;
pub const UPS_C_OVERCURRENT_INDICATOR: u16 = 0x0008;
pub const UPS_C_PORT_RESET: u16 = 0x0010;
pub const UPS_C_PORT_L1: u16 = 0x0020; // USB 2.0 only
pub const UPS_C_BH_PORT_RESET: u16 = 0x0020; // USB 3.0 only
pub const UPS_C_PORT_LINK_STATE: u16 = 0x0040;
pub const UPS_C_PORT_CONFIG_ERROR: u16 = 0x0080;

pub const UR_GET_STATUS: u8 = 0x00;
pub const UR_CLEAR_FEATURE: u8 = 0x01;
pub const UR_SET_FEATURE: u8 = 0x03;
pub const UR_SET_ADDRESS: u8 = 0x05;
pub const UR_GET_DESCRIPTOR: u8 = 0x06;
pub const UDESC_DEVICE: u8 = 0x01;
pub const UDESC_CONFIG: u8 = 0x02;
pub const UDESC_STRING: u8 = 0x03;
pub const USB_LANGUAGE_TABLE: u8 = 0x00; // Language ID string index
pub const UDESC_INTERFACE: u8 = 0x04;
pub const UDESC_ENDPOINT: u8 = 0x05;
pub const UDESC_DEVICE_QUALIFIER: u8 = 0x06;
pub const UDESC_OTHER_SPEED_CONFIGURATION: u8 = 0x07;
pub const UDESC_INTERFACE_POWER: u8 = 0x08;
pub const UDESC_OTG: u8 = 0x09;
pub const UDESC_DEBUG: u8 = 0x0A;
pub const UDESC_IFACE_ASSOC: u8 = 0x0B; // Interface association
pub const UDESC_BOS: u8 = 0x0F; // Binary object store
pub const UDESC_DEVICE_CAPABILITY: u8 = 0x10;
pub const UDESC_CS_DEVICE: u8 = 0x21; // Class specific
pub const UDESC_CS_CONFIG: u8 = 0x22;
pub const UDESC_CS_STRING: u8 = 0x23;
pub const UDESC_CS_INTERFACE: u8 = 0x24;
pub const UDESC_CS_ENDPOINT: u8 = 0x25;
pub const UDESC_HUB: u8 = 0x29;
pub const UDESC_SS_HUB: u8 = 0x2A; // Super speed
pub const UDESC_ENDPOINT_SS_COMP: u8 = 0x30; // Super speed
pub const UR_SET_DESCRIPTOR: u8 = 0x07;
pub const UR_GET_CONFIG: u8 = 0x08;
pub const UR_SET_CONFIG: u8 = 0x09;
pub const UR_GET_INTERFACE: u8 = 0x0A;
pub const UR_SET_INTERFACE: u8 = 0x0B;
pub const UR_SYNCH_FRAME: u8 = 0x0C;
pub const UR_SET_SEL: u8 = 0x30;
pub const UR_ISOCH_DELAY: u8 = 0x31;

// HUB specific requests
pub const UR_GET_BUS_STATE: u8 = 0x02;
pub const UR_CLEAR_TT_BUFFER: u8 = 0x08;
pub const UR_RESET_TT: u8 = 0x09;
pub const UR_GET_TT_STATE: u8 = 0x0A;
pub const UR_STOP_TT: u8 = 0x0B;
pub const UR_SET_AND_TEST: u8 = 0x0C; // USB 2.0 only
pub const UR_SET_HUB_DEPTH: u8 = 0x0C; // USB 3.0 only
pub const USB_SS_HUB_DEPTH_MAX: u8 = 5;
pub const UR_GET_PORT_ERR_COUNT: u8 = 0x0D;

pub const UDCLASS_IN_INTERFACE: u8 = 0x00;
pub const UDCLASS_COMM: u8 = 0x02;
pub const UDCLASS_HUB: u8 = 0x09;
pub const UDSUBCLASS_HUB: u8 = 0x00;
pub const UDPROTO_FSHUB: u8 = 0x00;
pub const UDPROTO_HSHUBSTT: u8 = 0x01;
pub const UDPROTO_HSHUBMTT: u8 = 0x02;
pub const UDPROTO_SSHUB: u8 = 0x03;
pub const UDCLASS_DIAGNOSTIC: u8 = 0xdc;
pub const UDCLASS_WIRELESS: u8 = 0xe0;
pub const UDSUBCLASS_RF: u8 = 0x01;
pub const UDPROTO_BLUETOOTH: u8 = 0x01;
pub const UDCLASS_VENDOR: u8 = 0xff;

pub const UICLASS_HUB: u8 = 0x09;
pub const UISUBCLASS_HUB: u8 = 0;
pub const UIPROTO_FSHUB: u8 = 0;
pub const UIPROTO_HSHUBSTT: u8 = 0; // Yes, same as previous
pub const UIPROTO_HSHUBMTT: u8 = 1;

pub const UF_ENDPOINT_HALT: u16 = 0;
pub const UF_DEVICE_REMOTE_WAKEUP: u16 = 1;
pub const UF_TEST_MODE: u16 = 2;
pub const UF_U1_ENABLE: u16 = 0x30;
pub const UF_U2_ENABLE: u16 = 0x31;
pub const UF_LTM_ENABLE: u16 = 0x32;

// HUB specific features
pub const UHF_C_HUB_LOCAL_POWER: u16 = 0;
pub const UHF_C_HUB_OVER_CURRENT: u16 = 1;
pub const UHF_PORT_CONNECTION: u16 = 0;
pub const UHF_PORT_ENABLE: u16 = 1;
pub const UHF_PORT_SUSPEND: u16 = 2;
pub const UHF_PORT_OVER_CURRENT: u16 = 3;
pub const UHF_PORT_RESET: u16 = 4;
pub const UHF_PORT_LINK_STATE: u16 = 5;
pub const UHF_PORT_POWER: u16 = 8;
pub const UHF_PORT_LOW_SPEED: u16 = 9;
pub const UHF_PORT_L1: u16 = 10;
pub const UHF_C_PORT_CONNECTION: u16 = 16;
pub const UHF_C_PORT_ENABLE: u16 = 17;
pub const UHF_C_PORT_SUSPEND: u16 = 18;
pub const UHF_C_PORT_OVER_CURRENT: u16 = 19;
pub const UHF_C_PORT_RESET: u16 = 20;
pub const UHF_PORT_TEST: u16 = 21;
pub const UHF_PORT_INDICATOR: u16 = 22;
pub const UHF_C_PORT_L1: u16 = 23;

// SuperSpeed HUB specific features
pub const UHF_PORT_U1_TIMEOUT: u16 = 23;
pub const UHF_PORT_U2_TIMEOUT: u16 = 24;
pub const UHF_C_PORT_LINK_STATE: u16 = 25;
pub const UHF_C_PORT_CONFIG_ERROR: u16 = 26;
pub const UHF_PORT_REMOTE_WAKE_MASK: u16 = 27;
pub const UHF_BH_PORT_RESET: u16 = 28;
pub const UHF_C_BH_PORT_RESET: u16 = 29;
pub const UHF_FORCE_LINKPM_ACCEPT: u16 = 30;

pub const UHD_PWR: u16 = 0x0003;
pub const UHD_PWR_GANGED: u16 = 0x0000;
pub const UHD_PWR_INDIVIDUAL: u16 = 0x0001;
pub const UHD_PWR_NO_SWITCH: u16 = 0x0002;
pub const UHD_COMPOUND: u16 = 0x0004;
pub const UHD_OC: u16 = 0x0018;
pub const UHD_OC_GLOBAL: u16 = 0x0000;
pub const UHD_OC_INDIVIDUAL: u16 = 0x0008;
pub const UHD_OC_NONE: u16 = 0x0010;
pub const UHD_TT_THINK: u16 = 0x0060;
pub const UHD_TT_THINK_8: u16 = 0x0000;
pub const UHD_TT_THINK_16: u16 = 0x0020;
pub const UHD_TT_THINK_24: u16 = 0x0040;
pub const UHD_TT_THINK_32: u16 = 0x0060;
pub const UHD_PORT_IND: u16 = 0x0080;


// SuperSpeed suspend support
pub const USB_INTERFACE_FUNC_SUSPEND: u16 = 0;
pub const USB_INTERFACE_FUNC_SUSPEND_LP: u16 = 1 << 8;
pub const USB_INTERFACE_FUNC_SUSPEND_RW: u16 = 1 << 9;


