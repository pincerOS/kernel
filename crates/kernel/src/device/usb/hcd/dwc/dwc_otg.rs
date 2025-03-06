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
use crate::device::usb::hcd::dwc::roothub::*;
use crate::device::usb::types::*;
use crate::device::usb::usbd::device::*;
use crate::device::usb::usbd::pipe::UsbPipeAddress;
use crate::device::usb::usbd::request::UsbDeviceRequest;
use crate::device::usb::usbd::usbd::*;

use crate::device::mailbox;
use crate::device::mailbox::PropSetPowerState;
use crate::device::system_timer::micro_delay;
use crate::memory;

pub const ChannelCount: usize = 16;

pub static mut dwc_otg_driver: DWC_OTG = DWC_OTG { base_addr: 0 };

/**
    \brief Prepares a channel to communicated with a device.

    Prepares a channel to communicated with the device specified in pipe.
*/
fn HcdPrepareChannel(
    device: &UsbDevice,
    channel: u8,
    length: u32,
    packet_id: PacketId,
    pipe: &UsbPipeAddress,
) -> ResultCode {
    let dwc_sc: &mut dwc_hub = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };

    // if channel > Core.Hardware.HostChannelCount {
    //     LOGF("HCD: Channel {} is not available on this host.\n", channel);
    //     return ErrorArgument;
    // }

    // Clear all existing interrupts.
    write_volatile(DOTG_HCINT(channel as usize), 0x3fff);

    // Program the channel.
    // ClearReg(&mut Host.Channel[channel as usize].Characteristic);
    dwc_sc.channel[channel as usize]
        .characteristics
        .DeviceAddress = pipe.device;
    dwc_sc.channel[channel as usize]
        .characteristics
        .EndPointNumber = pipe.end_point;
    dwc_sc.channel[channel as usize]
        .characteristics
        .EndPointDirection = pipe.direction;
    dwc_sc.channel[channel as usize].characteristics.LowSpeed = if pipe.speed == UsbSpeed::Low {
        true
    } else {
        false
    };

    dwc_sc.channel[channel as usize].characteristics.Type = pipe.transfer_type;
    dwc_sc.channel[channel as usize]
        .characteristics
        .MaximumPacketSize = size_to_number(pipe.max_size);
    dwc_sc.channel[channel as usize].characteristics.Enable = false;
    dwc_sc.channel[channel as usize].characteristics.Disable = false;

    let hcchar = convert_host_characteristics(dwc_sc.channel[channel as usize].characteristics);
    write_volatile(DOTG_HCCHAR(channel as usize), hcchar);

    // Clear split control.
    if pipe.speed != UsbSpeed::High {
        dwc_sc.channel[channel as usize].split_control.SplitEnable = true;
        if let Some(parent) = device.parent {
            unsafe {
                dwc_sc.channel[channel as usize].split_control.HubAddress = (*parent).number;
            }
        }
        dwc_sc.channel[channel as usize].split_control.PortAddress = device.port_number;
    }

    let hcsplt = convert_host_split_control(dwc_sc.channel[channel as usize].split_control);
    write_volatile(DOTG_HCSPLT(channel as usize), hcsplt);

    dwc_sc.channel[channel as usize].transfer_size.TransferSize = length;
    if pipe.speed == UsbSpeed::Low {
        dwc_sc.channel[channel as usize].transfer_size.PacketCount = (length + 7) / 8;
    } else {
        dwc_sc.channel[channel as usize].transfer_size.PacketCount = (length
            + dwc_sc.channel[channel as usize]
                .characteristics
                .MaximumPacketSize as u32
            - 1)
            / dwc_sc.channel[channel as usize]
                .characteristics
                .MaximumPacketSize as u32;
    }

    if dwc_sc.channel[channel as usize].transfer_size.PacketCount == 0 {
        dwc_sc.channel[channel as usize].transfer_size.PacketCount = 1;
    }

    dwc_sc.channel[channel as usize].transfer_size.packet_id = packet_id;
    let hctsiz = convert_host_transfer_size(dwc_sc.channel[channel as usize].transfer_size);
    write_volatile(DOTG_HCTSIZ(channel as usize), hctsiz);

    return ResultCode::OK;
}

pub fn HcdTransmitChannel(device: &UsbDevice, channel: u8, buffer: *mut u8) {
    unsafe {
        let dwc_sc: &mut dwc_hub = &mut *(device.soft_sc as *mut dwc_hub);
        let mut hcsplt = read_volatile(DOTG_HCSPLT(channel as usize));
        hcsplt &= !HCCHAR_CHENA;
        dwc_sc.channel[channel as usize].split_control.CompleteSplit = false;
        write_volatile(DOTG_HCSPLT(channel as usize), hcsplt);

        if ((buffer as u32) & 3) != 0 {
            println!(
                "HCD: Transfer buffer {:#x} is not DWORD aligned. Ignored, but dangerous.\n",
                buffer as u32,
            );
        }
        dwc_sc.channel[channel as usize].dma_address = buffer;
        write_volatile(DOTG_HCDMA(channel as usize), buffer as u32);

        let mut hcchar = read_volatile(DOTG_HCCHAR(channel as usize));
        hcchar |= HCCHAR_CHENA;
        hcchar |= HCCHAR_CHDIS;
        hcchar &= !(1 << 20 | 1 << 21);
        hcchar |= 1 << 20;

        dwc_sc.channel[channel as usize].characteristics.Enable = true;
        dwc_sc.channel[channel as usize].characteristics.Disable = false;
        dwc_sc.channel[channel as usize]
            .characteristics
            .PacketsPerFrame = 1;

        write_volatile(DOTG_HCCHAR(channel as usize), hcchar);
    }
}

fn HcdChannelInterruptToError(device: &mut UsbDevice, hcint: u32, isComplete: bool) -> ResultCode {
    // let dwc_sc: &mut dwc_hub = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };
    let mut result = ResultCode::OK;

    if hcint & HCINT_AHBERR != 0 {
        device.error = UsbTransferError::AhbError;
        println!("| HCD: AHB error on channel {}\n", device.last_transfer);
        return ResultCode::ErrorDevice;
    }

    if hcint & HCINT_STALL != 0 {
        device.error = UsbTransferError::Stall;
        println!("| HCD: Stall on channel {}\n", device.last_transfer);
        return ResultCode::ErrorDevice;
    }

    if hcint & HCINT_NAK != 0 {
        device.error = UsbTransferError::NoAcknowledge;
        println!("| HCD: NAK on channel {}\n", device.last_transfer);
        return ResultCode::ErrorDevice;
    }

    if hcint & HCINT_ACK == 0 {
        println!(
            "| HCD: ACK not received on channel {}\n",
            device.last_transfer
        );
        result = ResultCode::ErrorTimeout;
    }

    if hcint & HCINT_NYET != 0 {
        device.error = UsbTransferError::NotYetError;
        println!("| HCD: NYET on channel {}\n", device.last_transfer);
        return ResultCode::ErrorDevice;
    }

    if hcint & HCINT_BBLERR != 0 {
        device.error = UsbTransferError::Babble;
        println!("| HCD: Babble on channel {}\n", device.last_transfer);
        return ResultCode::ErrorDevice;
    }

    if hcint & HCINT_FRMOVRUN != 0 {
        device.error = UsbTransferError::BufferError;
        println!("| HCD: Frame overrun on channel {}\n", device.last_transfer);
        return ResultCode::ErrorDevice;
    }

    if hcint & HCINT_DATATGLERR != 0 {
        device.error = UsbTransferError::BitError;
        println!(
            "| HCD: Data toggle error on channel {}\n",
            device.last_transfer
        );
        return ResultCode::ErrorDevice;
    }

    if hcint & HCINT_XACTERR != 0 {
        device.error = UsbTransferError::ConnectionError;
        println!(
            "| HCD: Transaction error on channel {}\n",
            device.last_transfer
        );
        return ResultCode::ErrorDevice;
    }

    if hcint & HCINT_XFERCOMPL == 0 && isComplete {
        println!(
            "| HCD: Transfer not complete on channel {}\n",
            device.last_transfer
        );
        result = ResultCode::ErrorTimeout;
    }

    return result;
}

pub const RequestTimeout: u32 = 5000;
pub fn HcdChannelSendWaitOne(
    device: &mut UsbDevice,
    pipe: &mut UsbPipeAddress,
    channel: u8,
    buffer: *mut u8,
    bufferLength: u32,
    bufferOffset: u32,
    request: &mut UsbDeviceRequest,
) -> ResultCode {
    let mut result: ResultCode;
    let mut timeout: u32;
    let mut tries: u32 = 0;
    let mut globalTries: u32 = 0;
    let mut actualTries: u32 = 0;
    let dwc_sc: &mut dwc_hub = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };
    let mut hcint = 0;
    // Outer loop: run until either globalTries == 3 or actualTries == 10.
    while globalTries < 3 && actualTries < 10 {
        // Reset/prepare channel registers.
        write_volatile(DOTG_HCINT(channel as usize), 0x3fff);

        let hctsiz = read_volatile(DOTG_HCTSIZ(channel as usize));
        convert_into_host_transfer_size(
            hctsiz,
            &mut dwc_sc.channel[channel as usize].transfer_size,
        );

        let mut hcsplt = read_volatile(DOTG_HCSPLT(channel as usize));
        convert_into_host_split_control(
            hcsplt,
            &mut dwc_sc.channel[channel as usize].split_control,
        );

        // Transmit data.
        HcdTransmitChannel(device, channel, buffer.wrapping_add(bufferOffset as usize));

        timeout = 0;
        loop {
            if timeout == RequestTimeout {
                println!("HCD: Request to device has timed out.");
                device.error = UsbTransferError::ConnectionError;
                return ResultCode::ErrorTimeout;
            }
            timeout += 1;
            hcint = read_volatile(DOTG_HCINT(channel as usize));
            if hcint & HCINT_CHHLTD == 0 {
                micro_delay(10);
            } else {
                break;
            }
        }
        let hctsiz = read_volatile(DOTG_HCTSIZ(channel as usize));
        convert_into_host_transfer_size(
            hctsiz,
            &mut dwc_sc.channel[channel as usize].transfer_size,
        );
        hcint = read_volatile(DOTG_HCINT(channel as usize));

        if dwc_sc.channel[channel as usize].split_control.SplitEnable {
            if hcint & HCINT_ACK != 0 {
                // Try to complete the split up to 3 times.
                for tries_i in 0..3 {
                    tries = tries_i;
                    write_volatile(DOTG_HCINT(channel as usize), 0x3fff);

                    hcsplt = read_volatile(DOTG_HCSPLT(channel as usize));
                    hcsplt |= HCSPLT_COMPSPLT;
                    write_volatile(DOTG_HCSPLT(channel as usize), hcsplt);

                    dwc_sc.channel[channel as usize].split_control.CompleteSplit = true;
                    dwc_sc.channel[channel as usize].characteristics.Enable = true;
                    dwc_sc.channel[channel as usize].characteristics.Disable = false;

                    write_volatile(
                        DOTG_HCCHAR(channel as usize),
                        convert_host_characteristics(
                            dwc_sc.channel[channel as usize].characteristics,
                        ),
                    );

                    timeout = 0;
                    loop {
                        if timeout == RequestTimeout {
                            println!("HCD: Request split completion to ss has timed out.\n");
                            device.error = UsbTransferError::ConnectionError;
                            return ResultCode::ErrorTimeout;
                        }
                        timeout += 1;
                        hcint = read_volatile(DOTG_HCINT(channel as usize));
                        if hcint & HCINT_CHHLTD == 0 {
                            micro_delay(100);
                        } else {
                            break;
                        }
                    }

                    if hcint & HCINT_NYET == 0 {
                        break;
                    }
                }

                if tries == 3 {
                    micro_delay(25000);
                    globalTries = globalTries.wrapping_add(1);
                    actualTries = actualTries.wrapping_add(1);
                    continue;
                } else if hcint & HCINT_NAK != 0 {
                    globalTries = globalTries.wrapping_sub(1);
                    micro_delay(25000);
                    globalTries = globalTries.wrapping_add(1);
                    actualTries = actualTries.wrapping_add(1);
                    continue;
                } else if hcint & HCINT_XACTERR != 0 {
                    micro_delay(25000);
                    globalTries = globalTries.wrapping_add(1);
                    actualTries = actualTries.wrapping_add(1);
                    continue;
                }

                result = HcdChannelInterruptToError(device, hcint, false);
                if result != ResultCode::OK {
                    // LOG_DEBUGF(
                    //     "HCD: Control message to %#x: %02x%02x%02x%02x %02x%02x%02x%02x.\n",
                    //     *(pipe as *const u32),
                    //     *((request as *const u8).offset(0)),
                    //     *((request as *const u8).offset(1)),
                    //     *((request as *const u8).offset(2)),
                    //     *((request as *const u8).offset(3)),
                    //     *((request as *const u8).offset(4)),
                    //     *((request as *const u8).offset(5)),
                    //     *((request as *const u8).offset(6)),
                    //     *((request as *const u8).offset(7)),
                    // );
                    println!("| HCD: Request split completion to failed.\n");
                    return result;
                }
            } else if hcint & HCINT_NAK != 0 {
                globalTries = globalTries.wrapping_sub(1);
                micro_delay(25000);
                globalTries = globalTries.wrapping_add(1);
                actualTries = actualTries.wrapping_add(1);
                continue;
            } else if hcint & HCINT_XACTERR != 0 {
                micro_delay(25000);
                globalTries = globalTries.wrapping_add(1);
                actualTries = actualTries.wrapping_add(1);
                continue;
            }
        } else {
            result = HcdChannelInterruptToError(
                device,
                hcint,
                !dwc_sc.channel[channel as usize].split_control.SplitEnable,
            );
            if result != ResultCode::OK {
                // LOG_DEBUGF(
                //     "HCD: Control message to %#x: %02x%02x%02x%02x %02x%02x%02x%02x.\n",
                //     *(pipe as *const u32),
                //     *((request as *const u8).offset(0)),
                //     *((request as *const u8).offset(1)),
                //     *((request as *const u8).offset(2)),
                //     *((request as *const u8).offset(3)),
                //     *((request as *const u8).offset(4)),
                //     *((request as *const u8).offset(5)),
                //     *((request as *const u8).offset(6)),
                //     *((request as *const u8).offset(7)),
                // );
                println!("HCD: Request to failed.\n");
                return ResultCode::ErrorRetry;
            }
        }

        break;
    }

    if globalTries == 3 || actualTries == 10 {
        println!("HCD: Request to s has failed 3 times.\n");
        result = HcdChannelInterruptToError(
            device,
            hcint,
            !dwc_sc.channel[channel as usize].split_control.SplitEnable,
        );
        if result != ResultCode::OK {
            // LOG_DEBUGF(
            //     "HCD: Control message to %#x: %02x%02x%02x%02x %02x%02x%02x%02x.\n",
            //     *(pipe as *const u32),
            //     *((request as *const u8).offset(0)),
            //     *((request as *const u8).offset(1)),
            //     *((request as *const u8).offset(2)),
            //     *((request as *const u8).offset(3)),
            //     *((request as *const u8).offset(4)),
            //     *((request as *const u8).offset(5)),
            //     *((request as *const u8).offset(6)),
            //     *((request as *const u8).offset(7)),
            // );
            println!("HCD: Request to failed.\n");
            return result;
        }
        device.error = UsbTransferError::ConnectionError;
        return ResultCode::ErrorTimeout;
    }

    ResultCode::OK
}

fn HcdChannelSendWait(
    device: &mut UsbDevice,
    pipe: &mut UsbPipeAddress,
    channel: u8,
    buffer: *mut u8,
    buffer_length: u32,
    request: &mut UsbDeviceRequest,
    packet_id: PacketId,
) -> ResultCode {
    let mut tries: u32 = 0;

    let dwc_sc: &mut dwc_hub = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };

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
            // packets = Host.Channel[channel as usize].TransferSize.PacketCount;
            packets = dwc_sc.channel[channel as usize].transfer_size.PacketCount;
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
            let hctsiz = read_volatile(DOTG_HCTSIZ(channel as usize));
            dwc_sc.channel[channel as usize].transfer_size.TransferSize = hctsiz & (0x7ffff);

            transfer = buffer_length - dwc_sc.channel[channel as usize].transfer_size.TransferSize;
            // If the packet count hasn’t changed, break out of the loop.
            if packets == dwc_sc.channel[channel as usize].transfer_size.PacketCount {
                break;
            }
            // Continue looping if there are still packets in progress.
            if dwc_sc.channel[channel as usize].transfer_size.PacketCount == 0 {
                break;
            }
        }

        // Check for a stuck transfer.
        if packets == dwc_sc.channel[channel as usize].transfer_size.PacketCount {
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

pub fn HcdSubmitControlMessage(
    device: &mut UsbDevice,
    pipe: UsbPipeAddress,
    buffer: *mut u8,
    buffer_length: u32,
    request: &mut UsbDeviceRequest,
) -> ResultCode {
    if pipe.device == RootHubDeviceNumber as u8 {
        return HcdProcessRootHubMessage(device, pipe, buffer, buffer_length, request);
    }
    let dwc_sc = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };

    device.error = UsbTransferError::Processing;
    device.last_transfer = 0;

    let mut tempPipe = UsbPipeAddress {
        max_size: pipe.max_size,
        speed: pipe.speed,
        end_point: pipe.end_point,
        device: pipe.device,
        transfer_type: UsbTransfer::Control,
        direction: UsbDirection::Out,
        _reserved: 0,
    };

    let mut result;
    result = HcdChannelSendWait(
        device,
        &mut tempPipe,
        0,
        buffer,
        8,
        request,
        PacketId::Setup,
    );
    if result != ResultCode::OK {
        println!("| HCD: Failed to send control message to device.\n");
        return result;
    }

    if !buffer.is_null() {
        if pipe.direction == UsbDirection::Out {
            memory_copy_buf(&mut dwc_sc.databuffer, buffer, buffer_length as usize);
        }
        tempPipe.speed = pipe.speed;
        tempPipe.device = pipe.device;
        tempPipe.end_point = pipe.end_point;
        tempPipe.max_size = pipe.max_size;
        tempPipe.transfer_type = UsbTransfer::Control;
        tempPipe.direction = pipe.direction;

        result = HcdChannelSendWait(
            device,
            &mut tempPipe,
            0,
            buffer,
            buffer_length,
            request,
            PacketId::Data1,
        );
        if result != ResultCode::OK {
            println!("| HCD: Failed to send data message to device.\n");
            return result;
        }

        let hctsiz = read_volatile(DOTG_HCTSIZ(0));
        dwc_sc.channel[0].transfer_size.TransferSize = hctsiz & 0x7ffff;
        if pipe.direction == UsbDirection::In {
            if dwc_sc.channel[0].transfer_size.TransferSize <= buffer_length {
                device.last_transfer = buffer_length - dwc_sc.channel[0].transfer_size.TransferSize;
            } else {
                device.last_transfer = buffer_length;
            }

            memory_copy(
                buffer,
                dwc_sc.databuffer.as_ptr(),
                device.last_transfer as usize,
            );
        } else {
            device.last_transfer = buffer_length;
        }
    }

    tempPipe.speed = pipe.speed;
    tempPipe.device = pipe.device;
    tempPipe.end_point = pipe.end_point;
    tempPipe.max_size = pipe.max_size;
    tempPipe.transfer_type = UsbTransfer::Control;
    if (buffer_length == 0) || pipe.direction == UsbDirection::Out {
        tempPipe.direction = UsbDirection::In;
    } else {
        tempPipe.direction = UsbDirection::Out;
    }

    result = HcdChannelSendWait(
        device,
        &mut tempPipe,
        0,
        buffer,
        0,
        request,
        PacketId::Data1,
    );
    if result != ResultCode::OK {
        println!("| HCD: Failed to send status message to device.\n");
        return result;
    }

    let hctsiz = read_volatile(DOTG_HCTSIZ(0));
    dwc_sc.channel[0].transfer_size.TransferSize = hctsiz & 0x7ffff;
    if dwc_sc.channel[0].transfer_size.TransferSize != 0 {
        println!("| HCD: warning non zero status transfer\n");
    }

    device.error = UsbTransferError::NoError;

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
    let mut dwc_sc = unsafe { &mut *bus.dwc_sc };

    println!("| HCD: Starting");

    write_volatile(DOTG_DCTL, 1 << 1);
    micro_delay(1000);


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
    gusbcfg |= GUSBCFG_FORCEHOSTMODE;
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


    // hport = read_volatile(DOTG_HPRT);
    // hport |= HPRT_PRTRST;
    // write_volatile(DOTG_HPRT, hport & (0x1f140 | 0x100));

    // micro_delay(50000);
    // hport &= !HPRT_PRTRST;
    // write_volatile(DOTG_HPRT, hport & (0x1f140 | 0x100));
;
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
    pub channel: [host_channel; ChannelCount],
}

//default
#[derive(Copy, Clone)]
pub struct host_channel {
    characteristics: host_characteristics,
    split_control: host_split_control,
    transfer_size: host_transfer_size,
    dma_address: *mut u8,
}

impl Default for host_channel {
    fn default() -> host_channel {
        Self {
            dma_address: core::ptr::null_mut(), // Override only this field
            characteristics: host_characteristics::default(),
            split_control: host_split_control::default(),
            transfer_size: host_transfer_size::default(),
        }
    }
}

#[derive(Default, Copy, Clone)]
pub struct host_characteristics {
    pub MaximumPacketSize: u16,
    pub EndPointNumber: u8,
    pub EndPointDirection: UsbDirection,
    pub LowSpeed: bool,
    pub Type: UsbTransfer,
    pub PacketsPerFrame: u8,
    pub DeviceAddress: u8,
    pub Disable: bool,
    pub Enable: bool,
}

pub fn convert_host_characteristics(chan: host_characteristics) -> u32 {
    return (chan.MaximumPacketSize as u32)
        | (chan.EndPointNumber as u32) << 11
        | (chan.EndPointDirection as u32) << 15
        | (chan.LowSpeed as u32) << 17
        | (chan.Type as u32) << 18
        | (chan.PacketsPerFrame as u32) << 21
        | (chan.DeviceAddress as u32) << 22
        | (chan.Disable as u32) << 30
        | (chan.Enable as u32) << 31;
}

#[derive(Default, Copy, Clone)]
pub struct host_split_control {
    pub HubAddress: u32,
    pub PortAddress: u8,
    pub CompleteSplit: bool,
    pub SplitEnable: bool,
}

pub fn convert_into_host_split_control(val: u32, chan: &mut host_split_control) {
    chan.PortAddress = (val >> 0) as u8;
    chan.HubAddress = (val >> 7) as u32;
    chan.CompleteSplit = ((val >> 16) & 0x1) != 0;
    chan.SplitEnable = ((val >> 31) & 0x1) != 0;
}

pub fn convert_host_split_control(chan: host_split_control) -> u32 {
    return (chan.PortAddress as u32) << 0
        | (chan.HubAddress as u32) << 7
        | (chan.CompleteSplit as u32) << 16
        | (chan.SplitEnable as u32) << 31;
}

#[derive(Default, Copy, Clone)]
pub struct host_transfer_size {
    pub TransferSize: u32,
    pub PacketCount: u32,
    pub packet_id: PacketId,
}

pub fn convert_into_host_transfer_size(val: u32, chan: &mut host_transfer_size) {
    chan.TransferSize = val & 0x7ffff;
    chan.PacketCount = (val >> 19) & 0x7ff;
    chan.packet_id = PacketId::from_u8(((val >> 29) & 0x3) as u8);
}

pub fn convert_host_transfer_size(chan: host_transfer_size) -> u32 {
    return (chan.TransferSize as u32)
        | (chan.PacketCount as u32) << 19
        | (chan.packet_id as u32) << 29;
}

impl dwc_hub {
    pub fn new() -> Self {
        Self {
            databuffer: [0; 1024],
            phy_initialised: false,
            channel: [host_channel::default(); ChannelCount],
        }
    }
}

#[repr(u8)]
#[derive(Default, Copy, Clone)]
enum PacketId {
    #[default]
    Data0 = 0,
    Data1 = 1,
    Data2 = 2,
    Setup = 3,
}

impl PacketId {
    pub fn from_u8(val: u8) -> PacketId {
        match val {
            0 => PacketId::Data0,
            1 => PacketId::Data1,
            2 => PacketId::Data2,
            3 => PacketId::Setup,
            _ => PacketId::Data0,
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
