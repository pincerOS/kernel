use core::result;

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
    let size = 8 + buffer_length;

    let mut buf = vec![0u8; size as usize];
    
    let mut tx_hdr1 = buffer_length as u32;
    let mut tx_hdr2 = 0u32;

    // if (tx_hdr1 //don't need padding
    let b1 = tx_hdr1.to_le_bytes();
    let b2 = tx_hdr2.to_le_bytes();
    buf[0..4].copy_from_slice(&b1);
    buf[4..8].copy_from_slice(&b2);


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
    // ax88179_led_setting(device);

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
        // val = axge_read_cmd_2(device, AXGE_ACCESS_PHY, 2, PHY_BMCR);
    // }

    // let mut reg = BMCR_RESET;

    // axge_miibus_writereg(device, 3, MII_BMCR, reg);

    // //wait 100 ms for it t ocomplete 
    // for _ in 0..100 {
    //     reg = axge_miibus_readreg(device, 3, MII_BMCR);
    //     if reg & BMCR_RESET == 0 {
    //         break;
    //     }
    //     micro_delay(1000);
    // }

    // reg &= !(BMCR_PDOWN | BMCR_ISO);
    // if axge_miibus_readreg(device, 3, MII_BMCR) != reg {
    //     axge_miibus_writereg(device, 3, MII_BMCR, reg);
    // }
    // micro_delay(ms_to_micro(1000));
    ax88179_reset(device);
    println!("| AXGE: PHY reset complete");
    ax88179_link_reset(device);
    println!("| AXGE: Link reset complete");

    let mut rxctl = (AX_RX_CTL_START | AX_RX_CTL_AB | AX_RX_CTL_IPE) | AX_RX_CTL_PRO;
    ax88179_write_cmd(device, AX_ACCESS_MAC, AX_RX_CTL,
        2, 2, &mut rxctl as *mut u16 as *mut u8);

    println!("| AXGE: multicast mode set");

    return ResultCode::OK;
}

//cmd -> cmd
//
pub fn ax88179_led_setting(device: &mut UsbDevice) {
    let mut tmp = GMII_PHY_PGSEL_EXT;
    println!("| AXGE: Setting LED settings");
    // ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
    //     GMII_PHY_PAGE_SELECT, 2, &tmp);
    axge_write_cmd_2(device, AXGE_ACCESS_PHY, GMII_PHY_PGSEL_EXT,3, tmp);
    tmp = 0x2c;

    axge_write_cmd_2(device, AXGE_ACCESS_PHY, GMII_PHYPAGE as u16, 3, tmp);

    let mut ledact = axge_read_cmd_2(device, AXGE_ACCESS_PHY, GMII_LED_ACT as u16, 3);
    let mut ledlink = axge_read_cmd_2(device, AXGE_ACCESS_PHY, GMII_LED_LINK as u16, 3);

    ledact &= GMII_LED_ACTIVE_MASK;
	ledlink &= GMII_LED_LINK_MASK;

    ledact |= GMII_LED0_ACTIVE | GMII_LED1_ACTIVE | GMII_LED2_ACTIVE;
    ledlink |= GMII_LED0_LINK_10 | GMII_LED1_LINK_100 | GMII_LED2_LINK_1000;

    axge_write_cmd_2(device, AXGE_ACCESS_PHY, GMII_LED_ACT as u16, 3, ledact);
    axge_write_cmd_2(device, AXGE_ACCESS_PHY, GMII_LED_LINK as u16, 3, ledlink);

    tmp = GMII_PHY_PGSEL_PAGE0;

    axge_write_cmd_2(device, AXGE_ACCESS_PHY, GMII_PHY_PAGE_SELECT as u16, 3, tmp);

    let ledfd = 0x10 | 0x04 | 0x01;
    axge_write_cmd_1(device, AXGE_ACCESS_MAC, 0x73, ledfd);

    println!("| AXGE: LED settings complete");
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
    axge_write_cmd_1(device, AXGE_ACCESS_MAC, AXGE_CRCR as u16, CRCR_IP | CRCR_TCP | CRCR_UDP);
    axge_write_cmd_1(device, AXGE_ACCESS_MAC, AXGE_CTCR as u16, CRCR_IP | CRCR_TCP | CRCR_UDP);
}

pub fn axge_rxfilter(debice: &mut UsbDevice) {
    println!("| AXGE: Setting RX filter");
    // let mut rxmode = RCR_DROP_CRCERR | RCR_START | RCR_ACPT_BCAST | RCR_ACPT_ALL_MCAST;
    let rxmode = RCR_START | RCR_ACPT_BCAST | RCR_ACPT_ALL_MCAST;
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

pub fn ax88179_auto_detach(dev: &mut UsbDevice) {
    let mut tmp16: u16 = 0;
	let mut tmp8: u8 = 0;

	// if ax88179_read_cmd(dev, AX_ACCESS_EEPROM, 0x43, 1, 2, &mut tmp16  as *mut u16 as *mut u8) < 0 {
	// 	return;
    // }

	if ((tmp16 == 0xFFFF) || ((tmp16 & 0x0100) == 0)) {
		return;
    }
    unsafe {
        /* Enable Auto Detach bit */
        tmp8 = 0;
        ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_CLK_SELECT as u16, 1, 1, &mut tmp8 as *mut u8);
        tmp8 |= AX_CLK_SELECT_ULR;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_CLK_SELECT as u16, 1, 1, &mut tmp8 as *mut u8);

        ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL as u16, 2, 2, &mut tmp16 as *mut u16 as *mut u8);
        tmp16 |= AX_PHYPWR_RSTCTL_AT;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL as u16, 2, 2, &mut tmp16 as *mut u16 as *mut u8);
    }
}

pub fn ax88179_reset(dev: &mut UsbDevice) {

    let mut buf = [0u8; 6];
    let mut tmp16 = buf.as_mut_ptr() as *mut u16;
    let mut tmp = buf.as_mut_ptr() as *mut u8;



	/* Power up ethernet PHY */
    unsafe {
        *tmp16 = 0;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL as u16, 2, 2, tmp16 as *mut u8);

        *tmp16 = AX_PHYPWR_RSTCTL_IPRL;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL  as u16, 2, 2, tmp16 as *mut u8);
        micro_delay(500);

        *tmp = AX_CLK_SELECT_ACS | AX_CLK_SELECT_BCS;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_CLK_SELECT as u16, 1, 1, tmp);
        micro_delay(200);

        /* Ethernet PHY Auto Detach*/
        ax88179_auto_detach(dev);

        /* Read MAC address from DTB or asix chip */
        // ax88179_get_mac_addr(dev);
        // memcpy(dev->net->perm_addr, dev->net->dev_addr, ETH_ALEN);
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_NODE_ID as u16, 6, 6,
            MAC_ADDRESS.as_mut_ptr());

        /* RX bulk configuration */
        // memcpy(tmp, &AX88179_BULKIN_SIZE[0], 5);
        // ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_BULKIN_QCTRL, 5, 5, tmp);

        // dev->rx_urb_size = 1024 * 20;

        *tmp = 0x34;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PAUSE_WATERLVL_LOW as u16, 1, 1, tmp);

        *tmp = 0x52;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PAUSE_WATERLVL_HIGH as u16,
                1, 1, tmp);

        /* Enable checksum offload */
        *tmp = AX_RXCOE_IP | AX_RXCOE_TCP | AX_RXCOE_UDP |
            AX_RXCOE_TCPV6 | AX_RXCOE_UDPV6;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RXCOE_CTL as u16, 1, 1, tmp);

        *tmp = AX_TXCOE_IP | AX_TXCOE_TCP | AX_TXCOE_UDP |
            AX_TXCOE_TCPV6 | AX_TXCOE_UDPV6;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_TXCOE_CTL as u16, 1, 1, tmp);

        /* Configure RX control register => start operation */
        *tmp16 = AX_RX_CTL_DROPCRCERR | AX_RX_CTL_IPE | AX_RX_CTL_START |
            AX_RX_CTL_AP | AX_RX_CTL_AMALL | AX_RX_CTL_AB;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_CTL as u16, 2, 2, tmp16 as *mut u8);

        *tmp = AX_MONITOR_MODE_PMETYPE | AX_MONITOR_MODE_PMEPOL |
            AX_MONITOR_MODE_RWMP;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MONITOR_MOD as u16, 1, 1, tmp);

        /* Configure default medium type => giga */
        *tmp16 = AX_MEDIUM_RECEIVE_EN | AX_MEDIUM_TXFLOW_CTRLEN |
            AX_MEDIUM_RXFLOW_CTRLEN | AX_MEDIUM_FULL_DUPLEX |
            AX_MEDIUM_GIGAMODE;
        ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE as u16, 2, 2, tmp16 as *mut u8);
    }

    ax88179_disable_eee(dev);
    ax88179_ethtool(dev);
    mii_nway_restart(dev);

}


pub fn ax88179_link_reset(dev: &mut UsbDevice) {
    println!("| AX88179: Resetting link");
    let mut tmp32: u32 = 0x40000000;

    use crate::sync::get_time;
    let timeout = get_time() / 1000 + 100;
    let mut mode: u16 = 0;
    while tmp32 & 0x40000000 != 0{
		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_CTL, 2, 2, &mut mode as *mut u16 as *mut u8);

        let mut temporary: u16 = AX_RX_CTL_DROPCRCERR | AX_RX_CTL_IPE | AX_RX_CTL_START |
        AX_RX_CTL_AP | AX_RX_CTL_AMALL | AX_RX_CTL_AB;
		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_CTL, 2, 2, &mut temporary as *mut u16 as *mut u8);

        /* link up, check the usb device control TX FIFO full or empty*/

		/*link up, check the usb device control TX FIFO full or empty*/
		ax88179_read_cmd(dev, 0x81, 0x8c, 0, 4, &mut tmp32 as *mut u32 as *mut u8);

        if get_time() / 1000 > timeout {
            println!("| AX88179: Link reset timeout");
            break;
        }
    }

    let mut link_sts: u8 = 0;
    let mut tmp16: u16 = 0;

    mode = AX_MEDIUM_RECEIVE_EN | AX_MEDIUM_TXFLOW_CTRLEN |
	       AX_MEDIUM_RXFLOW_CTRLEN;

	ax88179_read_cmd(dev, AX_ACCESS_MAC, PHYSICAL_LINK_STATUS as u16,
			 1, 1, &mut link_sts as *mut u8);

	ax88179_read_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
			 GMII_PHY_PHYSR, 2, &mut tmp16 as *mut u16 as *mut u8);

    if (tmp16 & GMII_PHY_PHYSR_FULL) != 0 {
        mode |= AX_MEDIUM_FULL_DUPLEX;
    }
    ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE as u16,
            2, 2, &mut mode as *mut u16 as *mut u8);
}

pub fn ax88179_disable_eee(dev: &mut UsbDevice) {
    let mut tmp16 = GMII_PHY_PGSEL_PAGE3;
    unsafe { 
        ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID as u16,
            GMII_PHY_PAGE_SELECT, 2, &mut tmp16 as *mut u16 as *mut u8);
    
        tmp16 = 0x3246;
        ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
                MII_PHYADDR, 2, &mut tmp16 as *mut u16 as *mut u8);
    
        tmp16 = GMII_PHY_PGSEL_PAGE0;
        ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
                GMII_PHY_PAGE_SELECT, 2, &mut tmp16 as *mut u16 as *mut u8);
    }
}

pub fn mii_nway_restart(dev: &mut UsbDevice) {
    let mut bmcr = ax88179_mdio_read(dev, MII_BMCR);

    if bmcr & 0x1000 == 0 {
        bmcr |= 0x0200;
        ax88179_mdio_write(dev, MII_BMCR, bmcr);
    }
}

pub fn ax88179_ethtool(dev: &mut UsbDevice) {
    let val = ax88179_phy_read_mmd_indirect(dev, MDIO_AN_EEE_ADV,
        MDIO_MMD_AN);

    let mut adv = 0;
    if val & MDIO_EEE_100TX != 0 {
        adv |= MDIO_EEE_100TX;//1 << ETHTOOL_LINK_MODE_100baseT_Full_BIT;
    }

    if val & MDIO_EEE_1000T != 0 {
        adv |= MDIO_EEE_1000T;//1 << ETHTOOL_LINK_MODE_1000baseT_Full_BIT;
    }

    if val &  MDIO_EEE_10GT != 0 {
        adv |= MDIO_EEE_10GT;//1 << ETHTOOL_LINK_MODE_10000baseT_Full_BIT;
    }

    if val & MDIO_EEE_1000KX != 0 {
        adv |= MDIO_EEE_1000KX;//1 << ETHTOOL_LINK_MODE_1000baseKX_Full_BIT;
    }

    if val & MDIO_EEE_10GKX4 != 0 {
        adv |= MDIO_EEE_10GKX4;//1 << ETHTOOL_LINK_MODE_10000baseKX4_Full_BIT;
    }

    if val & MDIO_EEE_10GKR != 0 {
        adv |= MDIO_EEE_10GKR;//1 << ETHTOOL_LINK_MODE_10000baseKR_Full_BIT;
    }

    ax88179_phy_write_mmd_indirect(dev, MDIO_AN_EEE_ADV,
        MDIO_MMD_AN, adv);

}

pub fn ax88179_phy_write_mmd_indirect(dev: &mut UsbDevice, prtad: u16, devad: u16, data: u16)
{
    ax88179_phy_mmd_indirect(dev, prtad, devad);
    let mut tmp16 = data;
    ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
    MII_MMD_DATA, 2, &mut tmp16 as *mut u16 as *mut u8);
}


pub fn ax88179_phy_read_mmd_indirect(dev: &mut UsbDevice, prtad: u16, devad: u16) -> u16 {
    ax88179_phy_mmd_indirect(dev, prtad, devad);
    let mut tmp16: u16 = 0;
    ax88179_read_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
        MII_MMD_DATA, 2, &mut tmp16 as *mut u16 as *mut u8);

    return tmp16;
}

pub fn ax88179_phy_mmd_indirect(dev: &mut UsbDevice, prtad: u16, devad: u16) {
    let mut tmp16 = devad;

    ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
        MII_MMD_CTRL, 2, &mut tmp16 as *mut u16 as *mut u8);

    tmp16 = prtad;
	ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
				MII_MMD_DATA, 2, &mut tmp16 as *mut u16 as *mut u8);

	tmp16 = devad | MII_MMD_CTRL_NOINCR;
	ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
				MII_MMD_CTRL, 2, &mut tmp16 as *mut u16 as *mut u8);
}

pub fn ax88179_mdio_read(dev: &mut UsbDevice, loc: u16) -> u16 {
    let mut tmp16: u16 = 0;

    // Read MDIO register
    ax88179_read_cmd(dev, AXGE_ACCESS_PHY, 3, loc, 2, &mut tmp16 as *mut u16 as *mut u8);
    return tmp16;
}

pub fn ax88179_mdio_write(dev: &mut UsbDevice, loc: u16, val: u16) {
    let mut tmp16: u16 = val;
    ax88179_write_cmd(dev, AXGE_ACCESS_PHY, 3, loc, 2, &mut tmp16 as *mut u16 as *mut u8);
}

pub fn ax88179_read_cmd(device: &mut UsbDevice, cmd: u8, value: u16, index: u16, size: u16, data: *mut u8) {
    if size == 2 {
        let mut buf: u16 = 0;
        __ax88179_read_cmd(device, cmd, value, index, size, &buf as *const u16 as *mut u8);
        buf.to_le_bytes();
        unsafe {
            *(data as *mut u16) = buf;
        }

    } else if size == 4 {
        let mut buf: u32 = 0;
        __ax88179_read_cmd(device, cmd, value, index, size, &buf as *const u32 as *mut u8);
        buf.to_le_bytes();
        unsafe {
            *(data as *mut u32) = buf;
        }
    } else {
        __ax88179_read_cmd(device, cmd, value, index, size, data);
    }
}

pub fn ax88179_write_cmd(device: &mut UsbDevice, cmd: u8, value: u16, index: u16, size: u16, data: *mut u8) {
    if size == 2 {
        let mut buf: u16;
        buf = unsafe { core::ptr::read(data as *const u16) };
        buf.to_le_bytes();

        __ax88179_write_cmd(device, cmd, value, index, size, &buf as *const u16 as *mut u8);
    } else {
        __ax88179_write_cmd(device, cmd, value, index, size, data);
    }
}

pub fn __ax88179_read_cmd(device: &mut UsbDevice, cmd: u8, value: u16, index: u16, size: u16, data: *mut u8) -> ResultCode {
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
            data,
            size as u32,
            &mut UsbDeviceRequest {
                request_type: 0xC0,
                request: command_to_usb_device_request(cmd),
                index: index as u16,
                value: value as u16,
                length: size as u16,
            },
            1000, // timeout
        )
    };

    if result != ResultCode::OK {
        print!("| AXGE: Failed to read command.\n");
        return result;
    }

    return result;
}

pub fn __ax88179_write_cmd(device: &mut UsbDevice, cmd: u8, value: u16, index: u16, size: u16, data: *mut u8) -> ResultCode {
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
            data,
            size as u32,
            &mut UsbDeviceRequest {
                request_type: 0x40,
                request: command_to_usb_device_request(cmd),
                index: index as u16,
                value: value as u16,
                length: size as u16,
            },
            1000, // timeout
        )
    };

    if result != ResultCode::OK {
        print!("| AXGE: Failed to write command.\n");
        return result;
    }

    return result;
}


// MDIO Manageable Devices (MMDs)
pub const MDIO_MMD_PMAPMD: u16 = 1;      // Physical Medium Attachment / Physical Medium Dependent
pub const MDIO_MMD_WIS: u16 = 2;          // WAN Interface Sublayer
pub const MDIO_MMD_PCS: u16 = 3;          // Physical Coding Sublayer
pub const MDIO_MMD_PHYXS: u16 = 4;        // PHY Extender Sublayer
pub const MDIO_MMD_DTEXS: u16 = 5;        // DTE Extender Sublayer
pub const MDIO_MMD_TC: u16 = 6;           // Transmission Convergence
pub const MDIO_MMD_AN: u16 = 7;           // Auto-Negotiation
pub const MDIO_MMD_POWER_UNIT: u16 = 13;  // PHY Power Unit
pub const MDIO_MMD_C22EXT: u16 = 29;      // Clause 22 extension
pub const MDIO_MMD_VEND1: u16 = 30;       // Vendor specific 1
pub const MDIO_MMD_VEND2: u16 = 31;       // Vendor specific 2

// Generic MII register addresses
pub const MII_BMCR: u16 = 0x00;         // Basic mode control register
pub const MII_BMSR: u16 = 0x01;         // Basic mode status register
pub const MII_PHYSID1: u16 = 0x02;      // PHYS ID 1
pub const MII_PHYSID2: u16 = 0x03;      // PHYS ID 2
pub const MII_ADVERTISE: u16 = 0x04;    // Advertisement control register
pub const MII_LPA: u16 = 0x05;          // Link partner ability register
pub const MII_EXPANSION: u16 = 0x06;    // Expansion register
pub const MII_CTRL1000: u16 = 0x09;     // 1000BASE-T control
pub const MII_STAT1000: u16 = 0x0a;     // 1000BASE-T status
pub const MII_MMD_CTRL: u16 = 0x0d;     // MMD Access Control Register
pub const MII_MMD_DATA: u16 = 0x0e;     // MMD Access Data Register
pub const MII_ESTATUS: u16 = 0x0f;      // Extended Status
pub const MII_DCOUNTER: u16 = 0x12;     // Disconnect counter
pub const MII_FCSCOUNTER: u16 = 0x13;   // False carrier counter
pub const MII_NWAYTEST: u16 = 0x14;     // N-way auto-negotiation test register
pub const MII_RERRCOUNTER: u16 = 0x15;  // Receive error counter
pub const MII_SREVISION: u16 = 0x16;    // Silicon revision
pub const MII_RESV1: u16 = 0x17;        // Reserved
pub const MII_LBRERROR: u16 = 0x18;     // Loopback, receive, bypass error
pub const MII_PHYADDR: u16 = 0x19;      // PHY address
pub const MII_RESV2: u16 = 0x1a;        // Reserved
pub const MII_TPISTATUS: u16 = 0x1b;    // TPI status for 10 Mbps
pub const MII_NCONFIG: u16 = 0x1c;      // Network interface config

// Basic Mode Control Register (BMCR) bitfields
pub const BMCR_RESV: u16 = 0x003f;         // Unused
pub const BMCR_SPEED1000: u16 = 0x0040;     // MSB of speed (1000 Mbps)
pub const BMCR_CTST: u16 = 0x0080;          // Collision test
pub const BMCR_FULLDPLX: u16 = 0x0100;      // Full duplex
pub const BMCR_ANRESTART: u16 = 0x0200;     // Auto-negotiation restart
pub const BMCR_ISOLATE: u16 = 0x0400;       // Isolate data paths from MII
pub const BMCR_PDOWN: u16 = 0x0800;         // Power down
pub const BMCR_ANENABLE: u16 = 0x1000;      // Enable auto-negotiation
pub const BMCR_SPEED100: u16 = 0x2000;      // Select 100 Mbps
pub const BMCR_LOOPBACK: u16 = 0x4000;      // TXD loopback
pub const BMCR_RESET: u16 = 0x8000;         // Reset to default
pub const BMCR_SPEED10: u16 = 0x0000;       // Select 10 Mbps

// MMD Access Control register fields
pub const MII_MMD_CTRL_DEVAD_MASK: u16 = 0x001f;   // Mask MMD DEVAD
pub const MII_MMD_CTRL_ADDR: u16 = 0x0000;          // Address
pub const MII_MMD_CTRL_NOINCR: u16 = 0x4000;        // No post increment
pub const MII_MMD_CTRL_INCR_RDWT: u16 = 0x8000;     // Post increment on reads & writes
pub const MII_MMD_CTRL_INCR_ON_WT: u16 = 0xC000;    // Post increment on writes only

// Generic MDIO register mappings (all as u16)
pub const MDIO_CTRL1: u16 = MII_BMCR as u16;         // Basic Mode Control Register
pub const MDIO_STAT1: u16 = MII_BMSR as u16;         // Basic Mode Status Register
pub const MDIO_DEVID1: u16 = MII_PHYSID1 as u16;     // Device Identifier 1
pub const MDIO_DEVID2: u16 = MII_PHYSID2 as u16;     // Device Identifier 2

pub const MDIO_SPEED: u16 = 4;                      // Speed ability
pub const MDIO_DEVS1: u16 = 5;                      // Devices in package
pub const MDIO_DEVS2: u16 = 6;
pub const MDIO_CTRL2: u16 = 7;                      // 10G control 2
pub const MDIO_STAT2: u16 = 8;                      // 10G status 2
pub const MDIO_PMA_TXDIS: u16 = 9;                  // 10G PMA/PMD transmit disable
pub const MDIO_PMA_RXDET: u16 = 10;                 // 10G PMA/PMD receive signal detect
pub const MDIO_PMA_EXTABLE: u16 = 11;               // 10G PMA/PMD extended ability
pub const MDIO_PKGID1: u16 = 14;                    // Package identifier 1
pub const MDIO_PKGID2: u16 = 15;                    // Package identifier 2

// Auto-Negotiation (AN) related
pub const MDIO_AN_ADVERTISE: u16 = 16;              // Auto-Negotiation advertisement (base page)
pub const MDIO_AN_LPA: u16 = 19;                    // Auto-Negotiation link partner ability (base page)
pub const MDIO_PCS_EEE_ABLE: u16 = 20;              // EEE Capability register
pub const MDIO_PCS_EEE_ABLE2: u16 = 21;             // EEE Capability register 2
pub const MDIO_PMA_NG_EXTABLE: u16 = 21;            // 2.5G/5G PMA/PMD extended ability
pub const MDIO_PCS_EEE_WK_ERR: u16 = 22;            // EEE wake error counter
pub const MDIO_PHYXS_LNSTAT: u16 = 24;              // PHY XGXS lane state

pub const MDIO_AN_EEE_ADV: u16 = 60;                // EEE advertisement
pub const MDIO_AN_EEE_LPABLE: u16 = 61;             // EEE link partner ability
pub const MDIO_AN_EEE_ADV2: u16 = 62;               // EEE advertisement 2
pub const MDIO_AN_EEE_LPABLE2: u16 = 63;            // EEE link partner ability 2

pub const MDIO_AN_CTRL2: u16 = 64;                  // Auto-Negotiation THP bypass request control

// EEE Supported / Advertisement / Link Partner Advertisement registers
// (same bit masks used across multiple registers)

// Old (user-visible) names
pub const MDIO_AN_EEE_ADV_100TX: u16 = 0x0002;    // Advertise 100TX EEE cap
pub const MDIO_AN_EEE_ADV_1000T: u16 = 0x0004;    // Advertise 1000T EEE cap

// New generic names (aliasing old names)
pub const MDIO_EEE_100TX: u16 = MDIO_AN_EEE_ADV_100TX;    // 100TX EEE cap
pub const MDIO_EEE_1000T: u16 = MDIO_AN_EEE_ADV_1000T;    // 1000T EEE cap

// Other EEE capabilities
pub const MDIO_EEE_10GT: u16 = 0x0008;    // 10GBASE-T EEE cap
pub const MDIO_EEE_1000KX: u16 = 0x0010;  // 1000BASE-KX EEE cap
pub const MDIO_EEE_10GKX4: u16 = 0x0020;  // 10GBASE-KX4 EEE cap
pub const MDIO_EEE_10GKR: u16 = 0x0040;   // 10GBASE-KR EEE cap
pub const MDIO_EEE_40GR_FW: u16 = 0x0100; // 40GBASE-R fast wake
pub const MDIO_EEE_40GR_DS: u16 = 0x0200; // 40GBASE-R deep sleep
pub const MDIO_EEE_100GR_FW: u16 = 0x1000; // 100GBASE-R fast wake
pub const MDIO_EEE_100GR_DS: u16 = 0x2000; // 100GBASE-R deep sleep

// Ethtool Link Mode Bit Positions
pub const ETHTOOL_LINK_MODE_10baseT_Half_BIT: u32 = 0;
pub const ETHTOOL_LINK_MODE_10baseT_Full_BIT: u32 = 1;
pub const ETHTOOL_LINK_MODE_100baseT_Half_BIT: u32 = 2;
pub const ETHTOOL_LINK_MODE_100baseT_Full_BIT: u32 = 3;
pub const ETHTOOL_LINK_MODE_1000baseT_Half_BIT: u32 = 4;
pub const ETHTOOL_LINK_MODE_1000baseT_Full_BIT: u32 = 5;
pub const ETHTOOL_LINK_MODE_Autoneg_BIT: u32 = 6;
pub const ETHTOOL_LINK_MODE_TP_BIT: u32 = 7;
pub const ETHTOOL_LINK_MODE_AUI_BIT: u32 = 8;
pub const ETHTOOL_LINK_MODE_MII_BIT: u32 = 9;
pub const ETHTOOL_LINK_MODE_FIBRE_BIT: u32 = 10;
pub const ETHTOOL_LINK_MODE_BNC_BIT: u32 = 11;
pub const ETHTOOL_LINK_MODE_10000baseT_Full_BIT: u32 = 12;
pub const ETHTOOL_LINK_MODE_Pause_BIT: u32 = 13;
pub const ETHTOOL_LINK_MODE_Asym_Pause_BIT: u32 = 14;
pub const ETHTOOL_LINK_MODE_2500baseX_Full_BIT: u32 = 15;
pub const ETHTOOL_LINK_MODE_Backplane_BIT: u32 = 16;
pub const ETHTOOL_LINK_MODE_1000baseKX_Full_BIT: u32 = 17;
pub const ETHTOOL_LINK_MODE_10000baseKX4_Full_BIT: u32 = 18;
pub const ETHTOOL_LINK_MODE_10000baseKR_Full_BIT: u32 = 19;
pub const ETHTOOL_LINK_MODE_10000baseR_FEC_BIT: u32 = 20;
pub const ETHTOOL_LINK_MODE_20000baseMLD2_Full_BIT: u32 = 21;
pub const ETHTOOL_LINK_MODE_20000baseKR2_Full_BIT: u32 = 22;
pub const ETHTOOL_LINK_MODE_40000baseKR4_Full_BIT: u32 = 23;
pub const ETHTOOL_LINK_MODE_40000baseCR4_Full_BIT: u32 = 24;
pub const ETHTOOL_LINK_MODE_40000baseSR4_Full_BIT: u32 = 25;
pub const ETHTOOL_LINK_MODE_40000baseLR4_Full_BIT: u32 = 26;
pub const ETHTOOL_LINK_MODE_56000baseKR4_Full_BIT: u32 = 27;
pub const ETHTOOL_LINK_MODE_56000baseCR4_Full_BIT: u32 = 28;
pub const ETHTOOL_LINK_MODE_56000baseSR4_Full_BIT: u32 = 29;
pub const ETHTOOL_LINK_MODE_56000baseLR4_Full_BIT: u32 = 30;
pub const ETHTOOL_LINK_MODE_25000baseCR_Full_BIT: u32 = 31;


// GMII PHY Specific Status Register (PHYSR)
pub const GMII_PHY_PHYSR: u16 = 0x11;

pub const GMII_PHY_PHYSR_SMASK: u16 = 0xc000;   // Speed mask
pub const GMII_PHY_PHYSR_GIGA: u16 = 0x8000;     // 1000 Mbps
pub const GMII_PHY_PHYSR_100: u16 = 0x4000;      // 100 Mbps
pub const GMII_PHY_PHYSR_FULL: u16 = 0x2000;     // Full duplex
pub const GMII_PHY_PHYSR_LINK: u16 = 0x0400;     // Link up


// pub const BMCR_RESET: u16 = 0x8000;
pub const BMCR_LOOP: u16 = 0x4000;
pub const BMCR_SPEED0: u16 = 0x2000;
pub const BMCR_AUTOEN: u16 = 0x1000;
// pub const BMCR_PDOWN: u16 = 0x0800;
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



// Utility macro in C: #define BIT(x) (1 << (x))
// In Rust (no_std), we just shift manually.

pub const GMII_LED_ACT: u8 = 0x1a;
pub const GMII_LED_ACTIVE_MASK: u16 = 0xff8f;
pub const GMII_LED0_ACTIVE: u16 = 1 << 4;
pub const GMII_LED1_ACTIVE: u16 = 1 << 5;
pub const GMII_LED2_ACTIVE: u16 = 1 << 6;

pub const GMII_LED_LINK: u8 = 0x1c;
pub const GMII_LED_LINK_MASK: u16 = 0xf888;
pub const GMII_LED0_LINK_10: u16 = 1 << 0;
pub const GMII_LED0_LINK_100: u16 = 1 << 1;
pub const GMII_LED0_LINK_1000: u16 = 1 << 2;
pub const GMII_LED1_LINK_10: u16 = 1 << 4;
pub const GMII_LED1_LINK_100: u16 = 1 << 5;
pub const GMII_LED1_LINK_1000: u16 = 1 << 6;
pub const GMII_LED2_LINK_10: u16 = 1 << 8;
pub const GMII_LED2_LINK_100: u16 = 1 << 9;
pub const GMII_LED2_LINK_1000: u16 = 1 << 10;

// LED control fields
pub const LED0_ACTIVE: u16 = 1 << 0;
pub const LED0_LINK_10: u16 = 1 << 1;
pub const LED0_LINK_100: u16 = 1 << 2;
pub const LED0_LINK_1000: u16 = 1 << 3;
pub const LED0_FD: u16 = 1 << 4;
pub const LED0_USB3_MASK: u16 = 0x001f;

pub const LED1_ACTIVE: u16 = 1 << 5;
pub const LED1_LINK_10: u16 = 1 << 6;
pub const LED1_LINK_100: u16 = 1 << 7;
pub const LED1_LINK_1000: u16 = 1 << 8;
pub const LED1_FD: u16 = 1 << 9;
pub const LED1_USB3_MASK: u16 = 0x03e0;

pub const LED2_ACTIVE: u16 = 1 << 10;
pub const LED2_LINK_10: u16 = 1 << 11;
pub const LED2_LINK_100: u16 = 1 << 12;
pub const LED2_LINK_1000: u16 = 1 << 13;
pub const LED2_FD: u16 = 1 << 14;
pub const LED_VALID: u16 = 1 << 15;
pub const LED2_USB3_MASK: u16 = 0x7c00;

// PHY Page select
pub const GMII_PHYPAGE: u16 = 0x1e;
pub const GMII_PHY_PAGE_SELECT: u16 = 0x1f;

pub const GMII_PHY_PGSEL_EXT: u16 = 0x0007;
pub const GMII_PHY_PGSEL_PAGE0: u16 = 0x0000;
pub const GMII_PHY_PGSEL_PAGE3: u16 = 0x0003;
pub const GMII_PHY_PGSEL_PAGE5: u16 = 0x0005;




// PHY ID and EEPROM
pub const AX88179_PHY_ID: u16 = 0x03;
pub const AX_EEPROM_LEN: u16 = 0x100;
pub const AX88179_EEPROM_MAGIC: u32 = 0x1790_0b95;

// Multicast filter settings
pub const AX_MCAST_FLTSIZE: u8 = 8;
pub const AX_MAX_MCAST: u8 = 64;

// Interrupt status
pub const AX_INT_PPLS_LINK: u32 = 1 << 16;

// RX header
pub const AX_RXHDR_L4_TYPE_MASK: u8 = 0x1c;
pub const AX_RXHDR_L4_TYPE_UDP: u8 = 4;
pub const AX_RXHDR_L4_TYPE_TCP: u8 = 16;
pub const AX_RXHDR_L3CSUM_ERR: u8 = 2;
pub const AX_RXHDR_L4CSUM_ERR: u8 = 1;
pub const AX_RXHDR_CRC_ERR: u32 = 1 << 29;
pub const AX_RXHDR_DROP_ERR: u32 = 1 << 31;

// Access types
pub const AX_ACCESS_MAC: u8 = 0x01;
pub const AX_ACCESS_PHY: u8 = 0x02;
pub const AX_ACCESS_EEPROM: u8 = 0x04;
pub const AX_ACCESS_EFUS: u8 = 0x05;
pub const AX_RELOAD_EEPROM_EFUSE: u8 = 0x06;

// Pause water level
pub const AX_PAUSE_WATERLVL_HIGH: u8 = 0x54;
pub const AX_PAUSE_WATERLVL_LOW: u8 = 0x55;

// Physical Link Status
pub const PHYSICAL_LINK_STATUS: u8 = 0x02;
pub const AX_USB_SS: u8 = 1 << 2;
pub const AX_USB_HS: u8 = 1 << 1;

// General Status
pub const GENERAL_STATUS: u8 = 0x03;
pub const AX_SECLD: u8 = 1 << 2;

// SROM (EEPROM) addresses
pub const AX_SROM_ADDR: u8 = 0x07;
pub const AX_SROM_CMD: u8 = 0x0a;
pub const EEP_RD: u8 = 1 << 2;
pub const EEP_BUSY: u8 = 1 << 4;
pub const AX_SROM_DATA_LOW: u8 = 0x08;
pub const AX_SROM_DATA_HIGH: u8 = 0x09;

// RX Control
pub const AX_RX_CTL: u16 = 0x0b;
pub const AX_RX_CTL_DROPCRCERR: u16 = 0x0100;
pub const AX_RX_CTL_IPE: u16 = 0x0200;
pub const AX_RX_CTL_START: u16 = 0x0080;
pub const AX_RX_CTL_AP: u16 = 0x0020;
pub const AX_RX_CTL_AM: u16 = 0x0010;
pub const AX_RX_CTL_AB: u16 = 0x0008;
pub const AX_RX_CTL_AMALL: u16 = 0x0002;
pub const AX_RX_CTL_PRO: u16 = 0x0001;
pub const AX_RX_CTL_STOP: u16 = 0x0000;

// Node ID, Multicast Filter Array
pub const AX_NODE_ID: u8 = 0x10;
pub const AX_MULFLTARY: u8 = 0x16;

// Medium Status Mode
pub const AX_MEDIUM_STATUS_MODE: u8 = 0x22;
pub const AX_MEDIUM_GIGAMODE: u16 = 0x0001;
pub const AX_MEDIUM_FULL_DUPLEX: u16 = 0x0002;
pub const AX_MEDIUM_EN_125MHZ: u16 = 0x0008;
pub const AX_MEDIUM_RXFLOW_CTRLEN: u16 = 0x0010;
pub const AX_MEDIUM_TXFLOW_CTRLEN: u16 = 0x0020;
pub const AX_MEDIUM_RECEIVE_EN: u16 = 0x0100;
pub const AX_MEDIUM_PS: u16 = 0x0200;
pub const AX_MEDIUM_JUMBO_EN: u16 = 0x8040;

// Monitor Mode
pub const AX_MONITOR_MOD: u8 = 0x24;
pub const AX_MONITOR_MODE_RWLC: u8 = 1 << 1;
pub const AX_MONITOR_MODE_RWMP: u8 = 1 << 2;
pub const AX_MONITOR_MODE_PMEPOL: u8 = 1 << 5;
pub const AX_MONITOR_MODE_PMETYPE: u8 = 1 << 6;

// GPIO Control
pub const AX_GPIO_CTRL: u8 = 0x25;
pub const AX_GPIO_CTRL_GPIO3EN: u8 = 1 << 7;
pub const AX_GPIO_CTRL_GPIO2EN: u8 = 1 << 6;
pub const AX_GPIO_CTRL_GPIO1EN: u8 = 1 << 5;

// PHY Power Reset Control
pub const AX_PHYPWR_RSTCTL: u8 = 0x26;
pub const AX_PHYPWR_RSTCTL_BZ: u16 = 0x0010;
pub const AX_PHYPWR_RSTCTL_IPRL: u16 = 0x0020;
pub const AX_PHYPWR_RSTCTL_AT: u16 = 0x1000;

// RX Bulk IN Queue Control
pub const AX_RX_BULKIN_QCTRL: u8 = 0x2e;

// Clock Select
pub const AX_CLK_SELECT: u8 = 0x33;
pub const AX_CLK_SELECT_BCS: u8 = 1 << 0;
pub const AX_CLK_SELECT_ACS: u8 = 1 << 1;
pub const AX_CLK_SELECT_ULR: u8 = 1 << 3;


// RX Checksum Offload Engine (COE) Control
pub const AX_RXCOE_CTL: u8 = 0x34;
pub const AX_RXCOE_IP: u8 = 0x01;
pub const AX_RXCOE_TCP: u8 = 0x02;
pub const AX_RXCOE_UDP: u8 = 0x04;
pub const AX_RXCOE_TCPV6: u8 = 0x20;
pub const AX_RXCOE_UDPV6: u8 = 0x40;

// TX Checksum Offload Engine (COE) Control
pub const AX_TXCOE_CTL: u8 = 0x35;
pub const AX_TXCOE_IP: u8 = 0x01;
pub const AX_TXCOE_TCP: u8 = 0x02;
pub const AX_TXCOE_UDP: u8 = 0x04;
pub const AX_TXCOE_TCPV6: u8 = 0x20;
pub const AX_TXCOE_UDPV6: u8 = 0x40;
