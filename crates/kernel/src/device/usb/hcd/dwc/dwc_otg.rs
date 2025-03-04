/******************************************************************************
*	hcd/dwc/designware20.c
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*	hcd/dwc/designware20.c contains code to control the DesignWare� Hi-Speed
*	USB 2.0 On-The-Go (HS OTG) Controller.
*
*	THIS SOFTWARE IS NOT AFFILIATED WITH NOR ENDORSED BY SYNOPSYS IP.
******************************************************************************/

use crate::device::mailbox::PropGetPowerState;
use crate::device::usb::hcd::dwc::dwc_otgreg::*;
use crate::device::usb::types::*;
use crate::device::usb::usbd::device::*;
use crate::device::usb::usbd::pipe::UsbPipeAddress;
use crate::device::usb::usbd::request::UsbDeviceRequest;
use crate::device::usb::usbd::usbd::*;
use crate::device::usb::hcd::dwc::roothub::*;

use crate::device::mailbox;
use crate::device::mailbox::PropSetPowerState;
use crate::device::system_timer::micro_delay;
use crate::memory;

pub static mut dwc_otg_driver: DWC_OTG = DWC_OTG { base_addr: 0 };

fn HcdChannelSendWait(
    device: &mut UsbDevice,
    pipe: &mut UsbPipeAddress,
    channel: u8,
    buffer: *mut u8,
    buffer_length: u32,
    request: &mut UsbDeviceRequest,
    packet_id: u8,
) -> ResultCode {
    let mut tries: u32 = 0;

    loop {
        // Check for timeout after three attempts.
        if tries == 3 {
            println!("HCD: Failed to send to packet after 3 attempts.\n");
            return ResultCode::ErrorTimeout;
        }
        tries += 1;

        // Prepare the channel.
        let result = HcdPrepareChannel(device, channel, buffer_length, packet_id, pipe);
        if result != ResultCode::OK {
            device.error = UsbTransferError::ConnectionError;
            println!("HCD: Could not prepare data channel to packet.\n");
            return result;
        }

        let mut transfer: u32 = 0;
        // This variable will hold the previous packet count.
        let mut packets: u32;

        loop {
            // Read current packet count.
            packets = Host.Channel[channel as usize].TransferSize.PacketCount;
            let result = HcdChannelSendWaitOne(
                device,
                pipe,
                channel,
                buffer,
                buffer_length,
                transfer,
                request,
            );
            if result != ResultCode::OK {
                if result == ResultCode::ErrorRetry {
                    // Restart the entire process on ErrorRetry.
                    continue;
                }
                return result;
            }

            // Update the transfer progress.
            ReadBackReg(&mut Host.Channel[channel as usize].TransferSize);
            transfer = buffer_length - Host.Channel[channel as usize].TransferSize.TransferSize;
            // If the packet count hasn’t changed, break out of the loop.
            if packets == Host.Channel[channel as usize].TransferSize.PacketCount {
                break;
            }
            // Continue looping if there are still packets in progress.
            if Host.Channel[channel as usize].TransferSize.PacketCount == 0 {
                break;
            }
        }

        // Check for a stuck transfer.
        if packets == Host.Channel[channel as usize].TransferSize.PacketCount {
            device.error = UsbTransferError::ConnectionError;
            println!("HCD: Transfer to device got stuck.\n");
            return ResultCode::ErrorDevice;
        }

        // if tries > 1 {
        //     LOGF("HCD: Transfer to {} succeeded on attempt {}/3.\n", UsbGetDescription(device), tries);
        // }
        return ResultCode::OK;
    }
}


pub fn HcdSubmitControlMessage(device: &mut UsbDevice, pipe: UsbPipeAddress, buffer: *mut u8, buffer_length: u32, request: &mut UsbDeviceRequest) -> ResultCode {
    if pipe.device == RootHubDeviceNumber as u8 {
        return HcdProcessRootHubMessage(device, pipe, buffer, buffer_length, request);
    }

    device.error = UsbTransferError::Processing;
    device.last_transfer = 0;

    let mut tempPipe = UsbPipeAddress {
        max_size: pipe.max_size,
        speed: pipe.speed,
        end_point: pipe.end_point,
        device: pipe.device,
        transfer_type: pipe.transfer_type,
        direction: pipe.direction,
        _reserved: 0,
    };




    return ResultCode::OK;
}

fn mbox_set_power_on() -> ResultCode {
    //https://elixir.bootlin.com/freebsd/v14.2/source/sys/arm/broadcom/bcm2835/bcm283x_dwc_fdt.c#L82
    let msg = PropSetPowerState {
        device_id: 0x03,
        state: 1 | (1 << 1),
    };

    // let msg_get = PropGetPowerState {
    //     device_id: 0x03,
    // };

    let mailbox_base = unsafe { memory::map_device(0xfe00b880) }.as_ptr();
    let mut mailbox = unsafe { mailbox::VideoCoreMailbox::init(mailbox_base) };
    //TODO: FIX THIS

    // let check = unsafe { mailbox.get_property::<PropGetPowerState>(msg_get) };
    // match check {
    //     Ok(output) => {
    //         println!("| HCD: Power state is {}", output.state);
    //     },
    //     Err(e) => {
    //         println!("| HCD ERROR: Power state check failed");
    //         return ResultCode::ErrorDevice;
    //     }
    // }

    let resp = unsafe { mailbox.get_property::<PropSetPowerState>(msg) };

    //TODO: Ignore on QEMU for now
    match resp {
        Ok(output) => {
            println!("| HCD: Power on successful {}", output.state);
        }
        Err(_) => {
            println!("| HCD ERROR: Power on failed");
            // return ResultCode::ErrorDevice;
            return ResultCode::OK;
        }
    }

    return ResultCode::OK;
}

/**
    \brief Triggers the core soft reset.

    Raises the core soft reset signal high, and then waits for the core to
    signal that it is ready again.
*/
pub fn HcdReset() -> ResultCode {
    let mut count = 0;
    let mut grstcl = read_volatile(DOTG_GRSTCTL);

    while (grstcl & GRSTCTL_AHBIDLE) == 0 {
        count += 1;
        if count > 0x100000 {
            println!("| HCD Reset ERROR: Device Hang");
            return ResultCode::ErrorDevice;
        }
        grstcl = read_volatile(DOTG_GRSTCTL);
    }

    grstcl |= GRSTCTL_CSFTRST;
    write_volatile(DOTG_GRSTCTL, grstcl);
    count = 0;

    while (grstcl & GRSTCTL_CSFTRST) != 0 && (grstcl & GRSTCTL_AHBIDLE) == 0 {
        count += 1;
        if count > 0x100000 {
            println!("| HCD Reset ERROR: Device Hang");
            return ResultCode::ErrorDevice;
        }
        grstcl = read_volatile(DOTG_GRSTCTL);
    }

    return ResultCode::OK;
}

/**
    \brief Triggers the fifo flush for a given fifo.

    Raises the core fifo flush signal high, and then waits for the core to
    signal that it is ready again.
*/
fn HcdTransmitFifoFlush(fifo: CoreFifoFlush) -> ResultCode {
    let rst = (fifo as u32) << GRSTCTL_TXFNUM_SHIFT | GRSTCTL_TXFFLSH;
    write_volatile(DOTG_GRSTCTL, rst);

    let mut count = 0;
    let mut rst_code = read_volatile(DOTG_GRSTCTL);

    while (rst_code & GRSTCTL_TXFFLSH) >> 5 != 0 {
        count += 1;
        if count > 0x100000 {
            println!("| HCD ERROR: TXFifo Flush Device Hang");
            return ResultCode::ErrorDevice;
        }
        rst_code = read_volatile(DOTG_GRSTCTL);
    }

    return ResultCode::OK;
}

/**
    \brief Triggers the receive fifo flush for a given fifo.

    Raises the core receive fifo flush signal high, and then waits for the core to
    signal that it is ready again.
*/
fn HcdReceiveFifoFlush() -> ResultCode {
    let rst = GRSTCTL_RXFFLSH;
    write_volatile(DOTG_GRSTCTL, rst);

    let mut count = 0;
    let mut rst_code = read_volatile(DOTG_GRSTCTL);
    while (rst_code & GRSTCTL_RXFFLSH) >> 4 != 0 {
        count += 1;
        if count > 0x100000 {
            println!("| HCD ERROR: RXFifo Flush Device Hang");
            return ResultCode::ErrorDevice;
        }
        rst_code = read_volatile(DOTG_GRSTCTL);
    }

    return ResultCode::OK;
}

pub fn HcdStart(bus: &mut UsbBus) -> ResultCode {
    let mut dwc_sc = &mut bus.dwc_sc;

    println!("| HCD: Starting");

    let mut gusbcfg = read_volatile(DOTG_GUSBCFG);
    gusbcfg &= !(GUSBCFG_ULPIEXTVBUSDRV | GUSBCFG_TERMSELDLPULSE);

    write_volatile(DOTG_GUSBCFG, gusbcfg);

    if HcdReset() != ResultCode::OK {
        return ResultCode::ErrorTimeout;
    }

    if dwc_sc.phy_initialised == false {
        dwc_sc.phy_initialised = true;

        //csub sets this as 1 but dwc documentation sets it as 0
        gusbcfg &= !GUSBCFG_ULPI_UTMI_SEL;
        gusbcfg &= !GUSBCFG_PHYIF;
        write_volatile(DOTG_GUSBCFG, gusbcfg);
        HcdReset();
    }

    gusbcfg = read_volatile(DOTG_GUSBCFG);
    //FSPhyType = Dedicated full-speed interface 2'b01
    //HSPhyType = UTMI+ 2'b01
    gusbcfg &= !(GUSBCFG_ULPIFSLS | GUSBCFG_ULPICLKSUSM);
    write_volatile(DOTG_GUSBCFG, gusbcfg);

    //Enable DMA
    let mut gahbcfg = read_volatile(DOTG_GAHBCFG);
    gahbcfg |= GAHBCFG_DMAEN;
    gahbcfg &= !(1 << 23);
    write_volatile(DOTG_GAHBCFG, gahbcfg);

    gusbcfg = read_volatile(DOTG_GUSBCFG);
    let cfg2 = read_volatile(DOTG_GHWCFG2) & 0b111;

    match cfg2 {
        0 => {
            //HNP_SRP_CAPABLE
            gusbcfg |= GUSBCFG_HNPCAP | GUSBCFG_SRPCAP;
        }
        1 | 3 | 5 => {
            //SRP_CAPABLE
            gusbcfg &= !GUSBCFG_HNPCAP;
            gusbcfg |= GUSBCFG_SRPCAP;
        }
        2 | 4 | 6 => {
            //NO_SRP_CAPABLE_DEVICE
            gusbcfg &= !GUSBCFG_HNPCAP;
            gusbcfg &= !GUSBCFG_SRPCAP;
        }
        _ => {
            println!("| HCD ERROR: Unsupported cfg2 value {}", cfg2);
            return ResultCode::ErrorIncompatible;
        }
    }
    write_volatile(DOTG_GUSBCFG, gusbcfg);

    write_volatile(DOTG_PCGCCTL, 0);

    let mut hcfg = read_volatile(DOTG_HCFG);
    //FSPhyType = Dedicated full-speed interface 2'b01
    //HSPhyType = UTMI+ 2'b01
    hcfg &= !HCFG_FSLSPCLKSEL_MASK;
    //Host clock: 30-60Mhz
    write_volatile(DOTG_HCFG, hcfg);

    hcfg = read_volatile(DOTG_HCFG);
    hcfg |= HCFG_FSLSSUPP; //Sets speed for FS/LS devices, no HS devices
    write_volatile(DOTG_HCFG, hcfg);

    // if (Host->Config.EnableDmaDescriptor ==
    // 	Core->Hardware.DmaDescription &&
    // 	(Core->VendorId & 0xfff) >= 0x90a) {
    // 	LOG_DEBUG("HCD: DMA descriptor: enabled.\n");
    // } else {
    // 	LOG_DEBUG("HCD: DMA descriptor: disabled.\n");
    // }/

    let cfg3 = read_volatile(DOTG_GHWCFG3);
    let fifo_size = cfg3 >> 16; //?

    println!("| HCD: fifo size: {}", fifo_size);

    write_volatile(DOTG_GRXFSIZ, fifo_size);
    write_volatile(DOTG_GNPTXFSIZ, fifo_size | (fifo_size << 16));
    write_volatile(DOTG_HPTXFSIZ, fifo_size | (fifo_size << 16));

    let mut gotgctl = read_volatile(DOTG_GOTGCTL);
    gotgctl |= GOTGCTL_HSTSETHNPEN;
    write_volatile(DOTG_GOTGCTL, gotgctl);

    if HcdTransmitFifoFlush(CoreFifoFlush::FlushAll) != ResultCode::OK {
        return ResultCode::ErrorDevice;
    }

    if HcdReceiveFifoFlush() != ResultCode::OK {
        return ResultCode::ErrorDevice;
    }

    let hcfg = read_volatile(DOTG_HCFG);
    if (hcfg & HCFG_MULTISEGDMA) == 0 {
        let num_hst_chans =
            (read_volatile(DOTG_GHWCFG2) & GHWCFG2_NUMHSTCHNL_MASK) >> GHWCFG2_NUMHSTCHNL_SHIFT;

        for channel in 0..num_hst_chans {
            let mut chan = read_volatile(DOTG_HCCHAR(channel as usize));
            chan |= HCCHAR_EPDIR_IN | HCCHAR_CHDIS;
            chan &= !HCCHAR_CHENA;
            write_volatile(DOTG_HCCHAR(channel as usize), chan);
        }

        // Halt channels to put them into known state.
        for channel in 0..num_hst_chans {
            let mut chan = read_volatile(DOTG_HCCHAR(channel as usize));
            chan |= HCCHAR_EPDIR_IN | HCCHAR_CHDIS | HCCHAR_CHENA;
            write_volatile(DOTG_HCCHAR(channel as usize), chan);

            let mut timeout = 0;
            chan = read_volatile(DOTG_HCCHAR(channel as usize));
            while (chan & HCCHAR_CHENA) != 0 {
                timeout += 1;
                if timeout > 0x100000 {
                    println!("| HCD Start ERROR: Channel {} failed to halt", channel);
                }
                chan = read_volatile(DOTG_HCCHAR(channel as usize));
            }
        }
    }

    let mut hport = read_volatile(DOTG_HPRT);
    if (hport & HPRT_PRTCONNSTS) == 0 {
        println!("| HCD Powering on port");
        hport |= HPRT_PRTPWR;
        write_volatile(DOTG_HPRT, hport & (0x1f140 | 0x1000));
    }

    hport = read_volatile(DOTG_HPRT);
    hport |= HPRT_PRTRST;
    write_volatile(DOTG_HPRT, hport & (0x1f140 | 0x100));

    micro_delay(50000);
    hport &= !HPRT_PRTRST;
    write_volatile(DOTG_HPRT, hport & (0x1f140 | 0x100));

    return ResultCode::OK;
}

pub fn HcdInitialize(bus: &mut UsbBus, base_addr: *mut ()) -> ResultCode {
    unsafe {
        dwc_otg_driver = DWC_OTG::init(base_addr);
    }

    println!("| HCD: Initializing");

    let vendor_id = read_volatile(DOTG_GSNPSID);
    let user_id = read_volatile(DOTG_GUID);

    if (vendor_id & 0xfffff000) != 0x4f542000 {
        println!(
            "| HCD ERROR: Vendor ID: 0x{:x}, User ID: 0x{:x}",
            vendor_id, user_id
        );

        return ResultCode::ErrorIncompatible;
    } else {
        println!(
            "| HCD: Vendor ID: 0x{:x}, User ID: 0x{:x}",
            vendor_id, user_id
        );
    }

    let cfg2 = read_volatile(DOTG_GHWCFG2);

    if (cfg2 >> GHWCFG2_OTGARCH_SHIFT) & 0b10 == 0 {
        println!(
            "| HCD ERROR: Architecture not internal DMA {}",
            (cfg2 >> GHWCFG2_OTGARCH_SHIFT) & 0b10
        );
        return ResultCode::ErrorIncompatible;
    }

    //High-Speed PHY Interfaces 1: UTMI+
    // I think that QEMU is not properly updating the cfg2 registers
    // if (cfg2 >> GHWCFG2_HSPHYTYPE_SHIFT) & 0b11 == 0 {
    //     //print hex cfg2
    //     println!("| HCD ERROR: High speed physical unsupported {:x}: {}", cfg2, (cfg2 >> GHWCFG2_HSPHYTYPE_SHIFT) & 0b11);
    //     return ResultCode::ErrorIncompatible;
    // }

    // let hcfg = read_volatile(DOTG_HCFG);

    let mut gahbcfg = read_volatile(DOTG_GAHBCFG);
    gahbcfg &= !GAHBCFG_GLBLINTRMSK;

    write_volatile(DOTG_GINTMSK, 0);
    write_volatile(DOTG_GAHBCFG, gahbcfg);

    if mbox_set_power_on() != ResultCode::OK {
        return ResultCode::ErrorDevice;
    }

    ResultCode::OK
}

pub fn read_volatile(reg: usize) -> u32 {
    unsafe { core::ptr::read_volatile((dwc_otg_driver.base_addr + reg) as *mut u32) }
}
pub fn write_volatile(reg: usize, val: u32) {
    unsafe { core::ptr::write_volatile((dwc_otg_driver.base_addr + reg) as *mut u32, val) }
}

pub fn dwc_otg_initialize_controller(base_addr: *mut ()) {
    unsafe {
        dwc_otg_driver = DWC_OTG::init(base_addr);
    }
}

struct DWC_OTG {
    base_addr: usize,
}

impl DWC_OTG {
    pub unsafe fn init(base_addr: *mut ()) -> Self {
        Self {
            base_addr: base_addr as usize,
        }
    }
}

pub struct dwc_hub {
    pub databuffer: [u8; 1024],
    pub phy_initialised: bool,
}

impl dwc_hub {
    pub fn new() -> Self {
        Self {
            databuffer: [0; 1024],
            phy_initialised: false,
        }
    }
}

#[repr(u8)]
enum CoreFifoFlush {
    FlushNonPeriodic = 0,
    FlushPeriodic1 = 1,
    FlushPeriodic2 = 2,
    FlushPeriodic3 = 3,
    FlushPeriodic4 = 4,
    FlushPeriodic5 = 5,
    FlushPeriodic6 = 6,
    FlushPeriodic7 = 7,
    FlushPeriodic8 = 8,
    FlushPeriodic9 = 9,
    FlushPeriodic10 = 10,
    FlushPeriodic11 = 11,
    FlushPeriodic12 = 12,
    FlushPeriodic13 = 13,
    FlushPeriodic14 = 14,
    FlushPeriodic15 = 15,
    FlushAll = 16,
}
