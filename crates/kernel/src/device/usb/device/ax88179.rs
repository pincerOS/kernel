use crate::device::system_timer::micro_delay;
use crate::device::usb::hcd::dwc::dwc_otg::ms_to_micro;
use crate::device::usb::types::*;
use crate::device::usb::usbd::device::*;
use crate::device::usb::usbd::pipe::*;
use crate::device::usb::usbd::request::*;
use crate::device::usb::usbd::usbd::UsbSendBulkMessage;
use crate::device::usb::UsbControlMessage;
use crate::device::usb::usbd::endpoint::UsbEndpointDevice;
use crate::device::usb::device::net::*;
use crate::device::usb::PacketId;
use alloc::boxed::Box;
use alloc::vec;

/**
 *
 * usb/device/ax88179.rs
 *  By Aaron Lo
 *   Based off the freeBSD driver if_axge.c
 */ 
//https://elixir.bootlin.com/freebsd/v14.2/source/sys/dev/usb/net/if_axge.c

const MAC_ADDRESS: [u8; 6] = [0x54, 0x52, 0x00, 0x12, 0x34, 0x56]; // TODO: TBD / make this dynamic

pub unsafe fn axge_send_packet(
    device: &mut UsbDevice,
    buffer: *mut u8,
    buffer_length: u32,
) -> ResultCode {
    let size = 4 + buffer_length;

    let mut buf = vec![0u8; size as usize];
    buf[0] = (buffer_length & 0xFF) as u8;
    buf[1] = ((buffer_length >> 8) & 0xFF) as u8;
    buf[2] = 1;
    buf[3] = 0;

    unsafe {
        core::ptr::copy_nonoverlapping(buffer, buf.as_mut_ptr().add(4), buffer_length as usize);
    }

    let endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();
    let pid = if endpoint_device.endpoint_pid[2] % 2 == 0 {
        PacketId::Data0
    } else {
        PacketId::Data1
    };

    endpoint_device.endpoint_pid[2] += 1;

    let result = unsafe {
        UsbSendBulkMessage(
            device,
            UsbPipeAddress {
                transfer_type: UsbTransfer::Bulk,
                speed: device.speed,
                end_point: 3, //TODO: check this
                device: device.number as u8,
                direction: UsbDirection::Out,
                max_size: size_from_number(512 as u32),
                _reserved: 0,
            },
            buf.into_boxed_slice(),
            size,
            pid,
            1, //TODO: Check this
            10
        )
    };

    if result != ResultCode::OK {
        print!("| AXGE: Failed to send packet.\n");
        return result;
    }
    return result;
}

pub unsafe fn axge_receive_packet(
    device: &mut UsbDevice,
    buffer: Box<[u8]>,
    buffer_length: u32,
) -> ResultCode {

    let endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();
    let pid = if endpoint_device.endpoint_pid[2] % 2 == 0 {
        PacketId::Data0
    } else {
        PacketId::Data1
    };

    endpoint_device.endpoint_pid[2] += 1;

    let result = unsafe {
        UsbSendBulkMessage(
            device,
            UsbPipeAddress {
                transfer_type: UsbTransfer::Bulk,
                speed: device.speed,
                end_point: 2,
                device: device.number as u8,
                direction: UsbDirection::In,
                max_size: size_from_number(512),
                _reserved: 0,
            },
            buffer,
            buffer_length,
            pid,
            2, //TODOI: check this
            10,
        )
    };

    if result != ResultCode::OK {
        print!("| RNDIS: Failed to receive packet message.\n");
        return result;
    }

    // return 4;
    return ResultCode::OK;
}

pub fn axge_init(device: &mut UsbDevice) -> ResultCode {

    axge_stop(device);

	axge_reset(device);

    println!("| AXGE: Initializing device");
    axge_write_mem(device, AXGE_ACCESS_MAC, ETHER_ADDR_LEN as u16, AXGE_NIDR as u16,
	    MAC_ADDRESS.as_mut_ptr(), ETHER_ADDR_LEN as u32);

	axge_write_cmd_1(device, AXGE_ACCESS_MAC, AXGE_PWLLR, 0x34);
	axge_write_cmd_1(device, AXGE_ACCESS_MAC, AXGE_PWLHR, 0x52);

    axge_csum_cfg(device);
    axge_rxfilter(device);

    /*
	 * XXX
	 * Controller supports wakeup on link change detection,
	 * magic packet and wakeup frame recpetion.  But it seems
	 * there is no framework for USB ethernet suspend/wakeup.
	 * Disable all wakeup functions.
	 */
    println!("| AXGE: Disabling wakeup functions");
	axge_write_cmd_1(device, AXGE_ACCESS_MAC, AXGE_MMSR as u16, 0);
	axge_read_cmd_1(device, AXGE_ACCESS_MAC, AXGE_MMSR as u16);

	/* Configure default medium type. */
    println!("| AXGE: Configuring default medium type");
	axge_write_cmd_2(device, AXGE_ACCESS_MAC, 2, AXGE_MSR, MSR_GM | MSR_FD |
	    MSR_RFC | MSR_TFC | MSR_RE);

    //     usbd_xfer_set_stall(sc->sc_xfer[AXGE_BULK_DT_WR]);

	// if_setdrvflagbits(ifp, IFF_DRV_RUNNING, 0);
	// /* Switch to selected media. */
	// axge_ifmedia_upd(ifp);
    // TODO: This is reseting the phy -> need to look into if thigns go wrong WARNING
    //This is my attempt at htis from gpt code -> no clue if it works

    //issue phy reset
    println!("| AXGE: Resetting PHY");
    // axge_write_cmd_2(device, AXGE_ACCESS_PHY, 2, PHY_BMCR, BMCR_RESET);
    
    // let mut val = axge_read_cmd_2(device, AXGE_ACCESS_PHY, 2, PHY_BMCR);
    // while val & BMCR_RESET != 0 {
    //     micro_delay(ms_to_micro(10));
    //     val = axge_read_cmd_2(device, AXGE_ACCESS_PHY, 2, PHY_BMCR);
    // }

        // axge_write_cmd_2(device, AXGE_ACCESS_PHY, 2, PHY_BMCR, BMCR_RESET);
    
    // let mut val = axge_read_cmd_2(device, AXGE_ACCESS_PHY, 2, PHY_BMCR);
    // while val & BMCR_RESET != 0 {
    //     micro_delay(ms_to_micro(10));
    //     val = axge_read_cmd_2(device, AXGE_ACCESS_PHY, 2, PHY_BMCR);
    // }

    // let mut reg = BMCR_RESET;

    // axge_miibus_writereg(device, 0, MII_BMCR, reg);

    // //wait 100 ms for it t ocomplete 
    // for _ in 0..100 {
    //     reg = axge_miibus_readreg(device, 0, MII_BMCR);
    //     if reg & BMCR_RESET == 0 {
    //         break;
    //     }
    //     micro_delay(1000);
    // }

    // reg &= !(BMCR_PDOWN | BMCR_ISO);
    // if axge_miibus_readreg(device, 0, MII_BMCR) != reg {
    //     axge_miibus_writereg(device, 0, MII_BMCR, reg);
    // }
    // micro_delay(ms_to_micro(100));

    println!("| AXGE: PHY reset complete");
    return ResultCode::OK;
}

pub fn axge_miibus_readreg(device: &mut UsbDevice, phy: u16, reg: u16) -> u16 {
    let val = axge_read_cmd_2(device, AXGE_ACCESS_PHY, reg, phy);
    return val;
}

pub fn axge_miibus_writereg(device: &mut UsbDevice, phy: u16, reg: u16, val: u16) {
    axge_write_cmd_2(device, AXGE_ACCESS_PHY, reg, phy, val);
}

pub fn axge_csum_cfg(device: &mut UsbDevice) {
    // Enable checksum offload
    println!("| AXGE: Enabling checksum offload");
    axge_write_cmd_1(device, AXGE_ACCESS_MAC, AXGE_CRCR as u16, 0);
    axge_write_cmd_1(device, AXGE_ACCESS_MAC, AXGE_CTCR as u16, 0);
}

pub fn axge_rxfilter(debice: &mut UsbDevice) {
    println!("| AXGE: Setting RX filter");
    let mut rxmode = RCR_DROP_CRCERR | RCR_START | RCR_ACPT_BCAST | RCR_ACPT_ALL_MCAST;

    axge_write_cmd_2(debice, AXGE_ACCESS_MAC, 2, AXGE_RCR as u16, rxmode);

}

pub fn axge_chip_init(device: &mut UsbDevice) {
    
    //power up ethernet phy
    axge_write_cmd_2(device, AXGE_ACCESS_MAC, 2, AXGE_EPPRCR, 0);
    axge_write_cmd_2(device, AXGE_ACCESS_MAC, 2, AXGE_EPPRCR, EPPRCR_IPRL);

    micro_delay(ms_to_micro(250));
    axge_write_cmd_1(device, AXGE_ACCESS_MAC, AXGE_CLK_SELECT as u16, AXGE_CLK_SELECT_ACS | AXGE_CLK_SELECT_BCS);
    micro_delay(ms_to_micro(100));

    axge_write_cmd_1(device, AXGE_FW_MODE, AXGE_FW_MODE_178A179, 0);
}

pub fn axge_reset(device: &mut UsbDevice) {
    //ignore the usbd_req_set_config for now
    println!("| AXGE: Resetting device");
    micro_delay(ms_to_micro(10));
    axge_chip_init(device);
}

pub fn axge_stop(device: &mut UsbDevice) {
    println!("| AXGE: Stopping device");
    let mut val = axge_read_cmd_2(device, AXGE_ACCESS_MAC as u8, 2, AXGE_MSR as u16);
    val &= !MSR_RE;
    axge_write_cmd_2(device, AXGE_ACCESS_MAC, 2, AXGE_MSR as u16, val);
}

fn axge_read_cmd_1(device: &mut UsbDevice, cmd: u8, reg: u16) -> u8 {
    let val: u8 = 0;
    axge_read_mem(device, cmd, 1, reg, &val as *const u8 as *mut u8, 1);
    return val;
}

fn axge_read_cmd_2(device: &mut UsbDevice, cmd: u8, index: u16, reg: u16) -> u16 {
    let val: u16 = 0;

    axge_read_mem(device, cmd, index, reg, &val as *const u16 as *mut u8, 2);

    return val;
}

fn axge_write_cmd_1(device: &mut UsbDevice, cmd: u8, reg: u16, val: u8) {
    axge_write_mem(device, cmd, 1, reg, &val as *const u8 as *mut u8, 1);
}

fn axge_write_cmd_2(device: &mut UsbDevice, cmd: u8, index: u16, reg: u16, val: u16) {
    axge_write_mem(device, cmd, index, reg, &val as *const u16 as *mut u8, 2);
}

fn axge_read_mem(device: &mut UsbDevice, cmd: u8, index: u16, val: u16, buf: *mut u8, len: u32) -> ResultCode {

    let result = unsafe {
        UsbControlMessage(
            device,
            UsbPipeAddress {
                transfer_type: UsbTransfer::Control,
                speed: device.speed,
                end_point: 0,
                device: device.number as u8,
                direction: UsbDirection::In,
                max_size: size_from_number(device.descriptor.max_packet_size0 as u32),
                _reserved: 0,
            },
            buf,
            len,
            &mut UsbDeviceRequest {
                request_type: 0xC0,
                request: command_to_usb_device_request(cmd),
                index: index as u16,
                value: val as u16,
                length: len as u16,
            },
            1000, // timeout
        )
    };

    if result != ResultCode::OK {
        print!("| AXGE: Failed to read memory.\n");
        return result;
    }

    return ResultCode::OK;
}


fn axge_write_mem(device: &mut UsbDevice, cmd: u8, index: u16, val: u16, buf: *mut u8, len: u32) -> ResultCode {

    let result = unsafe {
        UsbControlMessage(
            device,
            UsbPipeAddress {
                transfer_type: UsbTransfer::Control,
                speed: device.speed,
                end_point: 0,
                device: device.number as u8,
                direction: UsbDirection::Out,
                max_size: size_from_number(device.descriptor.max_packet_size0 as u32),
                _reserved: 0,
            },
            buf,
            len,
            &mut UsbDeviceRequest {
                request_type: 0x40,
                request: command_to_usb_device_request(cmd),
                index: index as u16,
                value: val as u16,
                length: len as u16,
            },
            1000, // timeout
        )
    };

    if result != ResultCode::OK {
        print!("| AXGE: Failed to write memory.\n");
        return result;
    }

    return ResultCode::OK;
}


// Basic mode control register (rw)
pub const MII_BMCR: u16 = 0x00;

pub const BMCR_RESET: u16 = 0x8000;
pub const BMCR_LOOP: u16 = 0x4000;
pub const BMCR_SPEED0: u16 = 0x2000;
pub const BMCR_AUTOEN: u16 = 0x1000;
pub const BMCR_PDOWN: u16 = 0x0800;
pub const BMCR_ISO: u16 = 0x0400;
pub const BMCR_STARTNEG: u16 = 0x0200;
pub const BMCR_FDX: u16 = 0x0100;
pub const BMCR_CTEST: u16 = 0x0080;
pub const BMCR_SPEED1: u16 = 0x0040;


pub const PHY_BMCR: u16 = 0x00; // Basic Mode Control Register

// Length of an Ethernet address
pub const ETHER_ADDR_LEN: u8 = 6;

// Length of the Ethernet type field
pub const ETHER_TYPE_LEN: u8 = 2;

// Length of the Ethernet CRC
pub const ETHER_CRC_LEN: u8 = 4;

// Ethernet header length: destination + source addresses + type
pub const ETHER_HDR_LEN: u8 = ETHER_ADDR_LEN * 2 + ETHER_TYPE_LEN;

// Minimum frame length, including CRC
pub const ETHER_MIN_LEN: u16 = 64;

// Maximum frame length, including CRC
pub const ETHER_MAX_LEN: u16 = 1518;

// Maximum jumbo frame length, including CRC
pub const ETHER_MAX_LEN_JUMBO: u16 = 9018;

// Length of 802.1Q VLAN encapsulation
pub const ETHER_VLAN_ENCAP_LEN: u8 = 4;

// Mbuf adjust factor to force 32-bit alignment of IP header
pub const ETHER_ALIGN: u8 = 2;


// Access Registers
pub const AXGE_ACCESS_MAC: u8 = 0x01;
pub const AXGE_ACCESS_PHY: u8 = 0x02;
pub const AXGE_ACCESS_WAKEUP: u8 = 0x03;
pub const AXGE_ACCESS_EEPROM: u8 = 0x04;
pub const AXGE_ACCESS_EFUSE: u8 = 0x05;
pub const AXGE_RELOAD_EEPROM_EFUSE: u8 = 0x06;
pub const AXGE_FW_MODE: u8 = 0x08;
pub const AXGE_WRITE_EFUSE_EN: u8 = 0x09;
pub const AXGE_WRITE_EFUSE_DIS: u8 = 0x0A;
pub const AXGE_ACCESS_MFAB: u8 = 0x10;

// Firmware Modes
pub const AXGE_FW_MODE_178A179: u16 = 0x0000;
pub const AXGE_FW_MODE_179A: u16 = 0x0001;

// Physical Link Status Register
pub const AXGE_PLSR: u8 = 0x02;
pub const PLSR_USB_FS: u8 = 0x01;
pub const PLSR_USB_HS: u8 = 0x02;
pub const PLSR_USB_SS: u8 = 0x04;

// EEPROM Registers
pub const AXGE_EAR: u8 = 0x07;
pub const AXGE_EDLR: u8 = 0x08;
pub const AXGE_EDHR: u8 = 0x09;
pub const AXGE_ECR: u8 = 0x0A;

// Rx Control Register
pub const AXGE_RCR: u8 = 0x0B;
pub const RCR_STOP: u16 = 0x0000;
pub const RCR_PROMISC: u16 = 0x0001;
pub const RCR_ACPT_ALL_MCAST: u16 = 0x0002;
pub const RCR_AUTOPAD_BNDRY: u16 = 0x0004;
pub const RCR_ACPT_BCAST: u16 = 0x0008;
pub const RCR_ACPT_MCAST: u16 = 0x0010;
pub const RCR_ACPT_PHY_MCAST: u16 = 0x0020;
pub const RCR_START: u16 = 0x0080;
pub const RCR_DROP_CRCERR: u16 = 0x0100;
pub const RCR_IPE: u16 = 0x0200;
pub const RCR_TX_CRC_PAD: u16 = 0x0400;

// Node ID Register
pub const AXGE_NIDR: u8 = 0x10;

// Multicast Filter Array
pub const AXGE_MFA: u8 = 0x16;

// Medium Status Register
pub const AXGE_MSR: u16 = 0x22;
pub const MSR_GM: u16 = 0x0001;
pub const MSR_FD: u16 = 0x0002;
pub const MSR_EN_125MHZ: u16 = 0x0008;
pub const MSR_RFC: u16 = 0x0010;
pub const MSR_TFC: u16 = 0x0020;
pub const MSR_RE: u16 = 0x0100;
pub const MSR_PS: u16 = 0x0200;

// Monitor mode status register
pub const AXGE_MMSR: u8 = 0x24;

pub const MMSR_RWLC: u8 = 0x02;
pub const MMSR_RWMP: u8 = 0x04;
pub const MMSR_RWWF: u8 = 0x08;
pub const MMSR_RW_FLAG: u8 = 0x10;
pub const MMSR_PME_POL: u8 = 0x20;
pub const MMSR_PME_TYPE: u8 = 0x40;
pub const MMSR_PME_IND: u8 = 0x80;


// Ethernet PHY power & reset control register
pub const AXGE_EPPRCR: u16 = 0x26;
pub const EPPRCR_BZ: u16 = 0x0010;
pub const EPPRCR_IPRL: u16 = 0x0020;
pub const EPPRCR_AUTODETACH: u16 = 0x1000;

pub const AXGE_RX_BULKIN_QCTRL: u8 = 0x2e;

pub const AXGE_CLK_SELECT: u8 = 0x33;
pub const AXGE_CLK_SELECT_BCS: u8 = 0x01;
pub const AXGE_CLK_SELECT_ACS: u8 = 0x02;
pub const AXGE_CLK_SELECT_ACSREQ: u8 = 0x10;
pub const AXGE_CLK_SELECT_ULR: u8 = 0x08;

// COE Rx control register
pub const AXGE_CRCR: u8 = 0x34;
pub const CRCR_IP: u8 = 0x01;
pub const CRCR_TCP: u8 = 0x02;
pub const CRCR_UDP: u8 = 0x04;
pub const CRCR_ICMP: u8 = 0x08;
pub const CRCR_IGMP: u8 = 0x10;
pub const CRCR_TCPV6: u8 = 0x20;
pub const CRCR_UDPV6: u8 = 0x40;
pub const CRCR_ICMPV6: u8 = 0x80;

// COE Tx control register
pub const AXGE_CTCR: u8 = 0x35;
pub const CTCR_IP: u8 = 0x01;
pub const CTCR_TCP: u8 = 0x02;
pub const CTCR_UDP: u8 = 0x04;
pub const CTCR_ICMP: u8 = 0x08;
pub const CTCR_IGMP: u8 = 0x10;
pub const CTCR_TCPV6: u8 = 0x20;
pub const CTCR_UDPV6: u8 = 0x40;
pub const CTCR_ICMPV6: u8 = 0x80;

// Pause water level high register
pub const AXGE_PWLHR: u16 = 0x54;

// Pause water level low register
pub const AXGE_PWLLR: u16 = 0x55;

// Configuration number 1
pub const AXGE_CONFIG_IDX: u16 = 0;

// Interface index 0
pub const AXGE_IFACE_IDX: u16 = 0;
