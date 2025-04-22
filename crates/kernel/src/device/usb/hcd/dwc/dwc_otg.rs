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

use crate::device::usb::hcd::dwc::dwc_otgreg::*;
use crate::device::usb::hcd::dwc::roothub::*;
use crate::device::usb::types::*;
use crate::device::usb::usbd::device::*;
use crate::device::usb::usbd::pipe::UsbPipeAddress;
use crate::device::usb::usbd::request::UsbDeviceRequest;

use crate::device::gic;
use crate::device::mailbox::PropSetPowerState;
use crate::device::system_timer::micro_delay;
use crate::device::usb::usbd::endpoint::endpoint_descriptor;
use crate::device::usb::{UsbBulkMessage, UsbInterruptMessage, USB_TRANSFER_QUEUE};
use crate::device::MAILBOX;
use crate::event::context::Context;
use crate::event::schedule_rt;
use crate::shutdown;
use crate::sync::InterruptSpinLock;
use crate::sync::SpinLock;
use alloc::boxed::Box;

pub const ChannelCount: usize = 8;
pub static mut dwc_otg_driver: DWC_OTG = DWC_OTG { base_addr: 0 };

// The USB_TRANSFER_QUEUE will hold future USB transfer requests (Usb Xfer). A channel is a representation by the USB spec to be able to access an endpoint (the thing talking to the USB device).
// Callbacks are the method that should be invoked once the data has been transferred.
// USBD should first create a usb_xfer on the USB_TRANSFER_QUEUE and then see if a channel is available.
// if a channel is available, it will be assigned to the usb_xfer and the callback will be set.
//
// After the transfer is complete, the callback will be invoked. If there are pending transfers, the next transfer will be scheduled. otherwise, the channel will be freed.
pub static DWC_LOCK: InterruptSpinLock<DwcLock> = InterruptSpinLock::new(DwcLock::new());
pub static DWC_CHANNEL_ACTIVE: SpinLock<DwcChannelActive> = SpinLock::new(DwcChannelActive::new());
pub static mut DWC_CHANNEL_CALLBACK: DwcChannelCallback = DwcChannelCallback::new();

pub fn dwc_otg_register_interrupt_handler() {
    gic::GIC.get().register_isr(105, dwc_otg_interrupt_handler);
}

fn schedule_next_transfer(channel: u8) {
    //Check if another transfer is pending
    if let Some(transfer) = USB_TRANSFER_QUEUE.get_transfer() {
        //Enable transfer, keep holding channel

        let endpoint = &transfer.endpoint_descriptor;
        let device = unsafe { &mut *endpoint.device };

        //check if the transfer is a bulk transfer or endpoint transfer
        match transfer.as_ref().endpoint_descriptor.endpoint_type {
            //TODO: Kinda cursed, maybe make cleaner
            UsbTransfer::Bulk => unsafe {
                UsbBulkMessage(device, transfer, channel);
            },
            UsbTransfer::Interrupt => unsafe {
                UsbInterruptMessage(device, transfer, channel);
            },
            _ => {
                //Really should not be here
                panic!("DWC: Interrupt Handler Unsupported transfer type");
            }
        }
    } else {
        //Don't need the occupy channel anymore
        dwc_otg_free_channel(channel as u32);
    }
}

pub fn dwc_otg_interrupt_handler(_ctx: &mut Context, _irq: usize) {
    let mut hcint_channels = [0u32; ChannelCount];
    {
        //read interrupt status
        let status = read_volatile(DOTG_GINTSTS);
        //clear interrupt status
        write_volatile(DOTG_GINTSTS, status);

        if status & GINTSTS_HCHINT != 0 {
            let channels = read_volatile(DOTG_HAINT);
            for i in 0..ChannelCount {
                if channels & (1 << i) != 0 {
                    let _lock = DWC_LOCK.lock();
                    let hcint = read_volatile(DOTG_HCINT(i));
                    write_volatile(DOTG_HCINT(i), hcint);
                    hcint_channels[i] = hcint;
                }
            }
        }
    }

    {
        for i in 0..ChannelCount {
            if hcint_channels[i] != 0 {
                if let Some(endpoint_descriptor) =
                    unsafe { DWC_CHANNEL_CALLBACK.endpoint_descriptors[i] }
                {
                    if let Some(callback) = unsafe { DWC_CHANNEL_CALLBACK.callback[i] } {
                        let hcint = hcint_channels[i];
                        schedule_rt(move || {
                            callback(endpoint_descriptor, hcint, i as u8);
                            schedule_next_transfer(i as u8);
                        });
                    } else {
                        println!("| DWC: No callback for channel {}.\n", i);
                        shutdown();
                    }
                } else {
                    println!("| DWC: No endpoint descriptor for channel {}.\n", i);
                    shutdown();
                }
            }
        }
    }
}

pub fn DwcUpdateHostFrameInterval() {

    let hfir = read_volatile(DOTG_HFIR);
    println!("| DWC: HFIR: {:#x}", hfir);
    println!("| DWC: HFIR FRINT: {:#x}\n", hfir & HFIR_FRINT_MASK);
}

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
    // if pipe.device == 2 {
    //     dwc_sc.channel[channel as usize].characteristics.DeviceAddress = 2;
    // }
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
    // println!("| Channel type: {:#?} Channel endpoint num {:#?} Channel endpoint direction {:#?}", dwc_sc.channel[channel as usize].characteristics.Type, dwc_sc.channel[channel as usize].characteristics.EndPointNumber, dwc_sc.channel[channel as usize].characteristics.EndPointDirection);
    // println!("| Device adress {:#?}", dwc_sc.channel[channel as usize].characteristics.DeviceAddress);
    dwc_sc.channel[channel as usize]
        .characteristics
        .MaximumPacketSize = size_to_number(pipe.max_size);
    dwc_sc.channel[channel as usize].characteristics.Enable = false;
    dwc_sc.channel[channel as usize].characteristics.Disable = false;
    dwc_sc.channel[channel as usize].characteristics.OddFrame = false;
    dwc_sc.channel[channel as usize]
        .characteristics
        .PacketsPerFrame = 1;

    let hcchar = convert_host_characteristics(dwc_sc.channel[channel as usize].characteristics);
    write_volatile(DOTG_HCCHAR(channel as usize), hcchar);

    // Clear split control.
    dwc_sc.channel[channel as usize].split_control.HubAddress = 0;
    dwc_sc.channel[channel as usize].split_control.PortAddress = 0;
    dwc_sc.channel[channel as usize].split_control.XactPos = 0;
    dwc_sc.channel[channel as usize].split_control.CompleteSplit = false;
    dwc_sc.channel[channel as usize].split_control.SplitEnable = false;
    // if pipe.speed != UsbSpeed::High && device.parent.is_some() && unsafe { (*device.parent.unwrap()).speed == UsbSpeed::High  && (*device.parent.unwrap()).parent.is_some() }{
    if pipe.speed != UsbSpeed::High {
        dwc_sc.channel[channel as usize].split_control.SplitEnable = true;
        if let Some(parent) = device.parent {
            unsafe {
                // println!("| Parent number: {:#?}", (*parent).number);
                dwc_sc.channel[channel as usize].split_control.HubAddress = (*parent).number;
            }
        }
        // println!("| Port number: {:#?}", device.port_number);
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
    // println!("| HCTSIZE {:#x}\n", hctsiz);
    write_volatile(DOTG_HCTSIZ(channel as usize), hctsiz);

    return ResultCode::OK;
}

pub unsafe fn HcdTransmitChannel(device: &UsbDevice, channel: u8, buffer: *mut u8) {
    unsafe {
        let dwc_sc: &mut dwc_hub = &mut *(device.soft_sc as *mut dwc_hub);
        let hcsplt = read_volatile(DOTG_HCSPLT(channel as usize));
        convert_into_host_split_control(
            hcsplt,
            &mut dwc_sc.channel[channel as usize].split_control,
        );
        dwc_sc.channel[channel as usize].split_control.CompleteSplit = false;
        write_volatile(
            DOTG_HCSPLT(channel as usize),
            convert_host_split_control(dwc_sc.channel[channel as usize].split_control),
        );

        if ((buffer as usize) & 3) != 0 {
            println!(
                "HCD: Transfer buffer {:#x} is not DWORD aligned. Ignored, but dangerous.\n",
                buffer as usize,
            );
        }

        let dma_address = 0x2FF0000 + 0x1000 * channel as usize;
        let dma_loc = dwc_sc.dma_loc + 0x1000 * channel as usize;
        //copy from buffer to dma_loc for 32 bytes
        memory_copy(dma_loc as *mut u8, buffer, 100);

        crate::arch::memory::invalidate_physical_buffer_for_device(dma_loc as *mut (), 128);

        //print out the first 8 bytes stored in buffer
        // unsafe {
        //     println!("Buffer: {:#x} {:#x} {:#x} {:#x} {:#x} {:#x} {:#x} {:#x}", *(buffer), *((buffer as *const u8).offset(1)), *((buffer as *const u8).offset(2)), *((buffer as *const u8).offset(3)), *((buffer as *const u8).offset(4)), *((buffer as *const u8).offset(5)), *((buffer as *const u8).offset(6)), *((buffer as *const u8).offset(7)));
        // };

        let dma_buffer = (dma_address) as *mut u8;
        // let dma_buffer = buffer;
        // println!("Buffer address: {:#x}\n", buffer as usize);
        // println!("DMA buffer address: {:#x}\n", dma_buffer as usize);

        dwc_sc.channel[channel as usize].dma_address = dma_buffer;
        write_volatile(DOTG_HCDMA(channel as usize), dma_buffer as u32);

        let hcchar = read_volatile(DOTG_HCCHAR(channel as usize));
        convert_into_host_characteristics(
            hcchar,
            &mut dwc_sc.channel[channel as usize].characteristics,
        );

        dwc_sc.channel[channel as usize].characteristics.Enable = true;
        dwc_sc.channel[channel as usize].characteristics.Disable = false;
        dwc_sc.channel[channel as usize]
            .characteristics
            .PacketsPerFrame = 1;

        write_volatile(
            DOTG_HCCHAR(channel as usize),
            convert_host_characteristics(dwc_sc.channel[channel as usize].characteristics),
        );
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

pub const RequestTimeout: u32 = 50000;

pub fn HcdChannelSendWaitOne(
    device: &mut UsbDevice,
    pipe: &mut UsbPipeAddress,
    channel: u8,
    buffer: *mut u8,
    _bufferLength: u32,
    bufferOffset: u32,
    _request: &mut UsbDeviceRequest,
) -> ResultCode {
    let mut result: ResultCode;
    let mut timeout: u32;
    let mut tries: u32 = 0;
    let mut globalTries: u32 = 0;
    let actualTries: u32 = 0;
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
        unsafe { HcdTransmitChannel(device, channel, buffer.wrapping_add(bufferOffset as usize)) };
        timeout = 0;
        loop {
            if timeout == RequestTimeout {
                println!("| HCD: Request to device has timed out.");

                let hprt = read_volatile(DOTG_HPRT);
                let gintsts = read_volatile(DOTG_GINTSTS);
                let haint = read_volatile(DOTG_HAINT);
                let hcint = read_volatile(DOTG_HCINT(channel as usize));
                let hcchar = read_volatile(DOTG_HCCHAR(channel as usize));

                println!("| HCD hprt: {:#x}", hprt);
                println!("| HCD gintsts: {:#x}", gintsts);
                println!("| HCD haint: {:#x}", haint);
                println!("| HCD hcint: {:#x}", hcint);
                println!("| HCD hcchar: {:#x}", hcchar);
                println!("| HCD channel: {:#x}", channel);

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
        // println!("| HCD: Channel interrupt {:#x}\n", hcint);

        let hctsiz = read_volatile(DOTG_HCTSIZ(channel as usize));
        convert_into_host_transfer_size(
            hctsiz,
            &mut dwc_sc.channel[channel as usize].transfer_size,
        );
        hcint = read_volatile(DOTG_HCINT(channel as usize));

        if pipe.speed != UsbSpeed::High {
            if hcint & HCINT_ACK != 0 && dwc_sc.channel[channel as usize].split_control.SplitEnable
            {
                // Try to complete the split up to 3 times.
                println!("| HCD: Completing split to device with ACK\n");
                for tries_i in 0..3 {
                    tries = tries_i;
                    write_volatile(DOTG_HCINT(channel as usize), 0x3fff);

                    hcsplt = read_volatile(DOTG_HCSPLT(channel as usize));
                    convert_into_host_split_control(
                        hcsplt,
                        &mut dwc_sc.channel[channel as usize].split_control,
                    );
                    dwc_sc.channel[channel as usize].split_control.CompleteSplit = true;
                    write_volatile(
                        DOTG_HCSPLT(channel as usize),
                        convert_host_split_control(dwc_sc.channel[channel as usize].split_control),
                    );

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
                            println!("| HCD: Request split completion to ss has timed out.\n");
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
                    continue;
                } else if hcint & HCINT_NAK != 0 {
                    globalTries = globalTries.wrapping_sub(1);
                    micro_delay(25000);
                    continue;
                } else if hcint & HCINT_XACTERR != 0 {
                    micro_delay(25000);
                    continue;
                }

                let dma_buffer = read_volatile(DOTG_HCDMA(channel as usize));
                // let dma_ptr = (dwc_sc.dma_loc + (0x2FF0000 - dma_buffer) as usize) as *mut u8;
                let dma_ptr = dwc_sc.dma_loc as *mut u8;
                println!(
                    "dma_buffer2 {:#x} dma_ptr {:#x} dma_loc {:#x}",
                    dma_buffer, dma_ptr as usize, dwc_sc.dma_loc as usize
                );
                //print the first 8 bytes of the dma buffer
                unsafe {
                    println!(
                        "DMA Buffer2: {:#x} {:#x} {:#x} {:#x} {:#x} {:#x} {:#x} {:#x}",
                        *(dma_ptr),
                        *((dma_ptr as *const u8).offset(1)),
                        *((dma_ptr as *const u8).offset(2)),
                        *((dma_ptr as *const u8).offset(3)),
                        *((dma_ptr as *const u8).offset(4)),
                        *((dma_ptr as *const u8).offset(5)),
                        *((dma_ptr as *const u8).offset(6)),
                        *((dma_ptr as *const u8).offset(7))
                    );
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
                continue;
            } else if hcint & HCINT_XACTERR != 0 {
                micro_delay(25000);
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
                // shutdown();
                return ResultCode::ErrorRetry;
            }
        }

        break;
    }

    if globalTries == 3 || actualTries == 10 {
        println!("| HCD: Request to s has failed 3 times.\n");
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
            println!("| HCD: Request to failed.\n");
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

        let mut result;

        loop {
            // Read current packet count.
            // packets = Host.Channel[channel as usize].TransferSize.PacketCount;
            packets = dwc_sc.channel[channel as usize].transfer_size.PacketCount;
            result = HcdChannelSendWaitOne(
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
                    println!("| HCD: Retrying to packet.\n");
                    break;
                }
                println!("| DWC: Result is {:#?}", result);
                return result;
            }

            // Update the transfer progress.
            let hctsiz = read_volatile(DOTG_HCTSIZ(channel as usize));
            // dwc_sc.channel[channel as usize].transfer_size.TransferSize = hctsiz & (0x7ffff);
            // dwc_sc.channel[channel as usize].transfer_size.PacketCount = (hctsiz >> 19) & 0x3ff;
            convert_into_host_transfer_size(
                hctsiz,
                &mut dwc_sc.channel[channel as usize].transfer_size,
            );

            transfer = buffer_length - dwc_sc.channel[channel as usize].transfer_size.TransferSize;
            // println!("| HCD: Transfer to packet progress: {}/{} with packets {} from {}\n", transfer, buffer_length, dwc_sc.channel[channel as usize].transfer_size.PacketCount, packets);
            // If the packet count hasn’t changed, break out of the loop.
            if packets == dwc_sc.channel[channel as usize].transfer_size.PacketCount {
                // println!("| HCD: Transfer to packet got stuck.");
                break;
            }
            // Continue looping if there are still packets in progress.
            if dwc_sc.channel[channel as usize].transfer_size.PacketCount == 0 {
                // println!("| HCD: Transfer to packet completed.");
                break;
            }
        }

        if result == ResultCode::ErrorRetry {
            println!("| HCD: Retrying to packet.\n");
            continue;
        }

        // Check for a stuck transfer.
        if packets == dwc_sc.channel[channel as usize].transfer_size.PacketCount {
            //TODO: Hacky fix for a NAK on interrupt endpoint transfer
            let hcint = read_volatile(DOTG_HCINT(channel as usize));
            if hcint & HCINT_NAK != 0 {
                device.error = UsbTransferError::NoAcknowledge;
            } else {
                device.error = UsbTransferError::ConnectionError;
            }

            return ResultCode::ErrorDevice;
        }

        return ResultCode::OK;
    }
}

pub fn HcdUpdateTransferSize(device: &UsbDevice, channel: u8) -> u32 {
    unsafe {
        let dwc_sc: &mut dwc_hub = &mut *(device.soft_sc as *mut dwc_hub);
        let hctsiz = read_volatile(DOTG_HCTSIZ(channel as usize));
        convert_into_host_transfer_size(
            hctsiz,
            &mut dwc_sc.channel[channel as usize].transfer_size,
        );
        return dwc_sc.channel[channel as usize].transfer_size.TransferSize;
    }
}

fn HcdTransmitChannelNoWait(device: &UsbDevice, channel: u8, buffer: *mut u8) {
    unsafe {
        let dwc_sc: &mut dwc_hub = &mut *(device.soft_sc as *mut dwc_hub);
        let hcsplt = read_volatile(DOTG_HCSPLT(channel as usize));
        convert_into_host_split_control(
            hcsplt,
            &mut dwc_sc.channel[channel as usize].split_control,
        );
        dwc_sc.channel[channel as usize].split_control.CompleteSplit = false;
        write_volatile(
            DOTG_HCSPLT(channel as usize),
            convert_host_split_control(dwc_sc.channel[channel as usize].split_control),
        );

        if ((buffer as usize) & 3) != 0 {
            println!(
                "HCD: Transfer buffer in no wait {:#x} is not DWORD aligned. Ignored, but dangerous.\n",
                buffer as usize,
            );
        }

        dwc_sc.channel[channel as usize].dma_address = buffer;
        write_volatile(DOTG_HCDMA(channel as usize), buffer as u32);

        let hcchar = read_volatile(DOTG_HCCHAR(channel as usize));
        convert_into_host_characteristics(
            hcchar,
            &mut dwc_sc.channel[channel as usize].characteristics,
        );

        dwc_sc.channel[channel as usize].characteristics.Enable = true;
        dwc_sc.channel[channel as usize].characteristics.Disable = false;
        dwc_sc.channel[channel as usize]
            .characteristics
            .PacketsPerFrame = 1;

        write_volatile(
            DOTG_HCCHAR(channel as usize),
            convert_host_characteristics(dwc_sc.channel[channel as usize].characteristics),
        );
    }
}

fn HcdChannelSendOne(
    device: &mut UsbDevice,
    pipe: &mut UsbPipeAddress,
    channel: u8,
    buffer: *mut u8,
    bufferOffset: u32,
) -> ResultCode {
    let dwc_sc: &mut dwc_hub = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };
    write_volatile(DOTG_HCINT(channel as usize), 0x3fff);

    let haintmsk = read_volatile(DOTG_HAINTMSK);
    write_volatile(DOTG_HAINTMSK, haintmsk | (1 << channel));

    if pipe.transfer_type == UsbTransfer::Bulk && pipe.direction == UsbDirection::In {
        write_volatile(
            DOTG_HCINTMSK(channel as usize),
            HCINTMSK_XFERCOMPLMSK
                | HCINTMSK_CHHLTDMSK
                | HCINTMSK_AHBERRMSK
                | HCINTMSK_STALLMSK
                | HCINTMSK_ACKMSK
                | HCINTMSK_NYETMSK
                | HCINTMSK_XACTERRMSK
                | HCINTMSK_BBLERRMSK
                | HCINTMSK_FRMOVRUNMSK
                | HCINTMSK_DATATGLERRMSK,
        )
    } else {
        write_volatile(
            DOTG_HCINTMSK(channel as usize),
            HCINTMSK_XFERCOMPLMSK
                | HCINTMSK_CHHLTDMSK
                | HCINTMSK_AHBERRMSK
                | HCINTMSK_STALLMSK
                | HCINTMSK_NAKMSK
                | HCINTMSK_ACKMSK
                | HCINTMSK_NYETMSK
                | HCINTMSK_XACTERRMSK
                | HCINTMSK_BBLERRMSK
                | HCINTMSK_FRMOVRUNMSK
                | HCINTMSK_DATATGLERRMSK,
        );
    }

    let hctsiz = read_volatile(DOTG_HCTSIZ(channel as usize));
    convert_into_host_transfer_size(hctsiz, &mut dwc_sc.channel[channel as usize].transfer_size);

    let hcsplt = read_volatile(DOTG_HCSPLT(channel as usize));
    convert_into_host_split_control(hcsplt, &mut dwc_sc.channel[channel as usize].split_control);

    HcdTransmitChannelNoWait(device, channel, buffer.wrapping_add(bufferOffset as usize));

    return ResultCode::OK;
}

fn HcdChannelSend(
    device: &mut UsbDevice,
    pipe: &mut UsbPipeAddress,
    channel: u8,
    buffer: *mut u8,
    buffer_length: u32,
    packet_id: PacketId,
) -> ResultCode {
    // Prepare the channel.
    let result = HcdPrepareChannel(device, channel, buffer_length, packet_id, pipe);
    if result != ResultCode::OK {
        device.error = UsbTransferError::ConnectionError;
        println!("HCD: Could not prepare data channel to packet.\n");
        return result;
    }

    let result = HcdChannelSendOne(device, pipe, channel, buffer, 0);
    if result != ResultCode::OK {
        device.error = UsbTransferError::ConnectionError;
        println!("HCD: Could not send data to packet.\n");
        return result;
    }

    // Wait for the transfer to complete.

    return ResultCode::OK;
}

pub fn ReadHPRT() -> u32 {
    read_volatile(DOTG_HPRT)
}

pub unsafe fn HcdSubmitBulkMessage(
    device: &mut UsbDevice,
    channel: u8,
    pipe: UsbPipeAddress,
    buffer: Option<Box<[u8]>>,
    buffer_length: u32,
    packet_id: PacketId,
) -> ResultCode {
    let dwc_sc = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };
    device.error = UsbTransferError::Processing;
    device.last_transfer = 0;

    let mut tempPipe = UsbPipeAddress {
        max_size: pipe.max_size,
        speed: pipe.speed,
        end_point: pipe.end_point,
        device: pipe.device,
        transfer_type: UsbTransfer::Bulk,
        direction: pipe.direction,
        _reserved: 0,
    };

    if pipe.direction == UsbDirection::Out {
        let data_buffer = dwc_sc.dma_addr[channel as usize] as *mut u8;
        unsafe {
            memory_copy(
                data_buffer,
                buffer.unwrap().as_ptr(),
                buffer_length as usize,
            );
        }
    }

    let result = HcdChannelSend(
        device,
        &mut tempPipe,
        channel,
        dwc_sc.dma_phys[channel as usize] as *mut u8,
        buffer_length,
        packet_id,
    );

    if result != ResultCode::OK {
        return result;
    }

    device.error = UsbTransferError::NoError;
    return ResultCode::OK;
}

pub unsafe fn HcdSubmitInterruptMessage(
    device: &mut UsbDevice,
    channel: u8,
    pipe: UsbPipeAddress,
    buffer_length: u32,
    packet_id: PacketId,
) -> ResultCode {
    let dwc_sc = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };
    device.error = UsbTransferError::Processing;
    device.last_transfer = 0;

    let mut tempPipe = UsbPipeAddress {
        max_size: pipe.max_size,
        speed: pipe.speed,
        end_point: pipe.end_point,
        device: pipe.device,
        transfer_type: UsbTransfer::Interrupt,
        direction: UsbDirection::In,
        _reserved: 0,
    };

    let data_buffer = dwc_sc.dma_phys[channel as usize] as *mut u8;
    let result = HcdChannelSend(
        device,
        &mut tempPipe,
        channel,
        data_buffer,
        buffer_length,
        packet_id,
    );

    if result != ResultCode::OK {
        return result;
    }

    device.error = UsbTransferError::NoError;
    return ResultCode::OK;
}

pub unsafe fn HcdSubmitControlMessage(
    device: &mut UsbDevice,
    pipe: UsbPipeAddress,
    buffer: *mut u8,
    buffer_length: u32,
    request: &mut UsbDeviceRequest,
) -> ResultCode {
    // println!("| HcdSubmitControlMessage for device {}", pipe.device);
    let roothub_device_number = unsafe { (*device.bus).roothub_device_number };
    if pipe.device == roothub_device_number as u8 {
        return unsafe { HcdProcessRootHubMessage(device, pipe, buffer, buffer_length, request) };
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
    let request_buffer = request as *mut UsbDeviceRequest as *mut u8;

    // dwc_sc.channel[0].characteristics.MaximumPacketSize = 8;
    let mut result;
    result = HcdChannelSendWait(
        device,
        &mut tempPipe,
        0,
        request_buffer,
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
            unsafe {
                memory_copy(
                    dwc_sc.databuffer.as_mut_ptr(),
                    buffer,
                    buffer_length as usize,
                );
            }
        }
        tempPipe.speed = pipe.speed;
        tempPipe.device = pipe.device;
        tempPipe.end_point = pipe.end_point;
        tempPipe.max_size = pipe.max_size;
        tempPipe.transfer_type = UsbTransfer::Control;
        tempPipe.direction = pipe.direction;

        let data_buffer = dwc_sc.databuffer.as_mut_ptr();

        result = HcdChannelSendWait(
            device,
            &mut tempPipe,
            0,
            data_buffer,
            buffer_length,
            request,
            PacketId::Data1,
        );
        if result != ResultCode::OK {
            println!("| HCD: Coult not send data to device\n");
            return result;
        }

        let hctsiz = read_volatile(DOTG_HCTSIZ(0));
        // dwc_sc.channel[0].transfer_size.TransferSize = hctsiz & 0x7ffff;
        convert_into_host_transfer_size(hctsiz, &mut dwc_sc.channel[0].transfer_size);
        if pipe.direction == UsbDirection::In {
            if dwc_sc.channel[0].transfer_size.TransferSize <= buffer_length {
                device.last_transfer = buffer_length - dwc_sc.channel[0].transfer_size.TransferSize;
            } else {
                println!("| HCD: Weird transfer size\n");
                device.last_transfer = buffer_length;
            }
            unsafe {
                memory_copy(
                    dwc_sc.databuffer.as_mut_ptr(),
                    dwc_sc.dma_loc as *const u8,
                    device.last_transfer as usize,
                );

                memory_copy(
                    buffer,
                    dwc_sc.databuffer.as_ptr(),
                    device.last_transfer as usize,
                );
            }
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

    // tempPipe.direction = UsbDirection::In;
    //TODO: This is necessary in Real hardware I think but QEMU doesn't fully handle it
    //https://elixir.bootlin.com/qemu/v9.0.2/source/hw/usb/hcd-dwc2.c#L346
    result = HcdChannelSendWait(
        device,
        &mut tempPipe,
        0,
        dwc_sc.databuffer.as_mut_ptr(),
        0,
        request,
        PacketId::Data1,
    );
    if result != ResultCode::OK {
        // println!("| HCD: Could not send STATUS to device.");
        // return result;
    }

    let hctsiz = read_volatile(DOTG_HCTSIZ(0));
    convert_into_host_transfer_size(hctsiz, &mut dwc_sc.channel[0].transfer_size);
    if dwc_sc.channel[0].transfer_size.TransferSize != 0 {
        println!("| HCD: warning non zero status transfer");
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

    let resp;
    {
        let mut mailbox = MAILBOX.get().lock();
        resp = unsafe { mailbox.get_property::<PropSetPowerState>(msg) };
    }

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

    // grstcl |= GRSTCTL_CSFTRST;
    grstcl = GRSTCTL_CSFTRST;
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

fn DwcOtgTxFifoReset(value: u32) {
    write_volatile(DOTG_GRSTCTL, value);

    //wait for the reset to complete
    for _ in 0..16 {
        let value = read_volatile(DOTG_GRSTCTL);
        if (value & (GRSTCTL_TXFFLSH | GRSTCTL_RXFFLSH)) == 0 {
            break;
        }
    }
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

pub fn ms_to_micro(ms: u32) -> u32 {
    return ms * 1000;
}

pub fn init_fifo() -> ResultCode{
    let cfg3 = read_volatile(DOTG_GHWCFG3);
    let mut fifo_size = 4 * (cfg3 >> 16);

    let fifo_regs = 4 * 16;
    if fifo_size < fifo_regs {
        panic!("| HCD ERROR: FIFO size is too small");
    }

    fifo_size -= fifo_regs;
    fifo_size /= 2;
    fifo_size &= !3;

    write_volatile(DOTG_GRXFSIZ, fifo_size);
    let mut tx_start = fifo_size;
    fifo_size /= 2;

    write_volatile(DOTG_GNPTXFSIZ, ((fifo_size / 4) << 16) | (tx_start / 4));
    tx_start += fifo_size;

    write_volatile(DOTG_HPTXFSIZ, ((fifo_size / 4) << 16) | (tx_start / 4));

    write_volatile(DOTG_GINTMSK, GINTMSK_HCHINTMSK);

    if HcdTransmitFifoFlush(CoreFifoFlush::FlushAll) != ResultCode::OK {
        return ResultCode::ErrorDevice;
    }

    if HcdReceiveFifoFlush() != ResultCode::OK {
        return ResultCode::ErrorDevice;
    }

    ResultCode::OK
}

pub fn DwcInit(bus: &mut UsbBus, base_addr: *mut ()) -> ResultCode {
    unsafe {
        dwc_otg_driver = DWC_OTG::init(base_addr);
    }

    if mbox_set_power_on() != ResultCode::OK {
        return ResultCode::ErrorDevice;
    }

    let dwc_sc: &mut dwc_hub = &mut *bus.dwc_sc;

    unsafe {
        let dma_address = 0x2FF0000;
        use crate::memory::map_device_block;
        let dma_loc =
            // map_physical_noncacheable(dma_address, 0x1000 * ChannelCount).as_ptr() as usize;
            map_device_block(dma_address, 0x1000 * ChannelCount).as_ptr() as usize;
        dwc_sc.dma_loc = dma_loc; //TODO: Temporay, move to somwhere elses
        println!(
            "| HCD: DMA address {:#x} mapped from {:#x} to {:#x}",
            dma_address,
            dma_loc,
            dma_loc + 0x1000 * ChannelCount
        );
        for i in 0..ChannelCount {
            dwc_sc.dma_addr[i as usize] = dma_loc + (i * 0x1000);
            dwc_sc.dma_phys[i as usize] = dma_address + (i * 0x1000);
        }
    }

    dwc_otg_register_interrupt_handler();

    let vendor_id = read_volatile(DOTG_GSNPSID);
    let user_id = read_volatile(DOTG_GUID);

    println!("| HCD: Vendor ID: {:#x} User ID: {:#x}", vendor_id, user_id);

    let version = read_volatile( DOTG_GSNPSID);
    println!("| HCD: Version: {:#x}", version);

    //disconnect
    write_volatile(DOTG_DCTL, DCTL_SFTDISCON);

    //wait for disconnect
    micro_delay(ms_to_micro(32));

    write_volatile(DOTG_GRSTCTL, GRSTCTL_CSFTRST);

    //wait for reset
    micro_delay(ms_to_micro(10));

    write_volatile(DOTG_GUSBCFG, GUSBCFG_FORCEHOSTMODE);
    write_volatile(DOTG_GOTGCTL, 0);

    //clear global NAK
    write_volatile(DOTG_DCTL, DCTL_CGOUTNAK | DCTL_CGNPINNAK);

    //disable usb port
    write_volatile(DOTG_PCGCCTL, 0xFFFFFFFF);

    micro_delay(ms_to_micro(10));

    //enable usb port
    write_volatile(DOTG_PCGCCTL, 0);
    micro_delay(ms_to_micro(10));

    if init_fifo() != ResultCode::OK {
        println!("| HCD ERROR: Failed to init FIFO");
        return ResultCode::ErrorDevice;
    }

    //setup clock
    let mut hcfg = read_volatile(DOTG_HCFG);
    hcfg &= !(HCFG_FSLSSUPP | HCFG_FSLSPCLKSEL_MASK);
    hcfg |= (1 << HCFG_FSLSPCLKSEL_SHIFT) | HCFG_FSLSSUPP;
    //Host clock: 30-60Mhz
    write_volatile(DOTG_HCFG, hcfg);

    write_volatile(DOTG_GAHBCFG, GAHBCFG_GLBLINTRMSK);

    let mut hport = read_volatile(DOTG_HPRT);
    if (hport & HPRT_PRTPWR) == 0 {
        println!("| HCD Powering on port");
        hport |= HPRT_PRTPWR;
        write_volatile(DOTG_HPRT, hport);
        micro_delay(10000);
    }

    //Enable DMA
    let mut gahbcfg = read_volatile(DOTG_GAHBCFG);
    gahbcfg |= GAHBCFG_DMAEN | GAHBCFG_GLBLINTRMSK;
    gahbcfg &= !(1 << 23);
    write_volatile(DOTG_GAHBCFG, gahbcfg);

    let hcfg = read_volatile(DOTG_HCFG);
    let h_dmaen = hcfg & (1 << 23);
    let cfg4 = read_volatile(DOTG_GHWCFG4);
    let c_dmad = cfg4 & (1 << 31);
    let c_dmaen = cfg4 & (1 << 30);
    let gahbcfg = read_volatile(DOTG_GAHBCFG);
    let g_dmaen = gahbcfg & GAHBCFG_DMAEN;
    let gsnpsid = read_volatile(DOTG_GSNPSID);

    println!(
        "| HCD: DMA enabled: {}, DMA description: {}, dma en {}, gdmaen {}, GSNPSID: {:#x}",
        h_dmaen,
        c_dmad,
        c_dmaen,
        g_dmaen,
        gsnpsid & 0xfff
    );

    ResultCode::OK
}


pub fn HcdStart(bus: &mut UsbBus) -> ResultCode {
    let dwc_sc: &mut dwc_hub = &mut *bus.dwc_sc;

    unsafe {
        let dma_address = 0x2FF0000;
        use crate::memory::map_physical_noncacheable;
        let dma_loc =
            map_physical_noncacheable(dma_address, 0x1000 * ChannelCount).as_ptr() as usize;
        dwc_sc.dma_loc = dma_loc; //TODO: Temporay, move to somwhere elses
        println!(
            "| HCD: DMA address {:#x} mapped from {:#x} to {:#x}",
            dma_address,
            dma_loc,
            dma_loc + 0x1000 * ChannelCount
        );
        for i in 0..ChannelCount {
            dwc_sc.dma_addr[i as usize] = dma_loc + (i * 0x1000);
            dwc_sc.dma_phys[i as usize] = dma_address + (i * 0x1000);
        }
    }

    println!("| HCD: Start");

    write_volatile(DOTG_DCTL, 1 << 1);
    micro_delay(32000);

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
        println!("| HCD: Reset PHY");
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

    write_volatile(DOTG_PCGCCTL, 0xFFFFFFFF);

    micro_delay(30000);

    write_volatile(DOTG_PCGCCTL, 0);

    micro_delay(30000);

    let mut hcfg = read_volatile(DOTG_HCFG);
    //FSPhyType = Dedicated full-speed interface 2'b01
    //HSPhyType = UTMI+ 2'b01
    hcfg &= !(HCFG_FSLSSUPP | HCFG_FSLSPCLKSEL_MASK);
    hcfg |= (1 << HCFG_FSLSPCLKSEL_SHIFT) | HCFG_FSLSSUPP;
    //Host clock: 30-60Mhz
    write_volatile(DOTG_HCFG, hcfg);

    // hcfg = read_volatile(DOTG_HCFG);
    // hcfg |= HCFG_FSLSSUPP; //Sets speed for FS/LS devices, no HS devices
    // write_volatile(DOTG_HCFG, hcfg);

    let h_dmaen = hcfg & (1 << 23);
    let cfg4 = read_volatile(DOTG_GHWCFG4);
    let c_dmad = cfg4 & (1 << 31);
    let gsnpsid = read_volatile(DOTG_GSNPSID);

    println!(
        "| HCD: DMA enabled: {}, DMA description: {}, GSNPSID: {:#x}",
        h_dmaen,
        c_dmad,
        gsnpsid & 0xfff
    );

    // if (Host->Config.EnableDmaDescriptor ==
    // 	Core->Hardware.DmaDescription &&
    // 	(Core->VendorId & 0xfff) >= 0x90a) {
    // 	LOG_DEBUG("HCD: DMA descriptor: enabled.\n");
    // } else {
    // 	LOG_DEBUG("HCD: DMA descriptor: disabled.\n");
    // }/

    let cfg3 = read_volatile(DOTG_GHWCFG3);
    let fifo_size = cfg3 >> 16; //?

    // println!("| HCD: fifo size: {}", fifo_size);

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
            write_volatile(DOTG_HCINT(channel as usize), 0xFFFFFFFF);
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
            // chan = read_volatile(DOTG_HCCHAR(channel as usize));
            let mut chan_int = read_volatile(DOTG_HCINT(channel as usize));
            while (chan_int & HCINT_CHHLTD) == 0 {
                timeout += 1;
                if timeout > 0x100000 {
                    println!("| HCD Start ERROR: Channel {} failed to halt", channel);
                }
                chan_int = read_volatile(DOTG_HCINT(channel as usize));
            }
            // while (chan & HCCHAR_CHENA) != 0 {
            //     timeout += 1;
            //     if timeout > 0x100000 {
            //         println!("| HCD Start ERROR: Channel {} failed to halt", channel);
            //     }
            //     chan = read_volatile(DOTG_HCCHAR(channel as usize));
            // }
        }
    }

    let mut hport = read_volatile(DOTG_HPRT);
    if (hport & HPRT_PRTPWR) == 0 {
        println!("| HCD Powering on port");
        hport |= HPRT_PRTPWR;
        write_volatile(DOTG_HPRT, hport & (0x1f140 | 0x1000));
        micro_delay(10000);
    }

    // Wait for device connection (optional debounce)
    for _ in 0..100 {
        let h = read_volatile(DOTG_HPRT);
        if (h & HPRT_PRTCONNSTS) != 0 {
            break;
        }
        micro_delay(1000); // wait 1ms
    }

    write_volatile(DOTG_GINTSTS, 0xFFFFFFFF);
    

    micro_delay(10000);
    write_volatile(DOTG_GINTMSK, GINTMSK_HCHINTMSK);

    write_volatile(DOTG_GINTSTS, 1);
    write_volatile(DOTG_GAHBCFG, GAHBCFG_GLBLINTRMSK);


    // hport = read_volatile(DOTG_HPRT);
    // hport |= HPRT_PRTRST;
    // write_volatile(DOTG_HPRT, hport);

    // micro_delay(50000);
    // hport &= !HPRT_PRTRST;
    // write_volatile(DOTG_HPRT, hport);
    // micro_delay(50000);

    return ResultCode::OK;
}

pub fn HcdInitialize(_bus: &mut UsbBus, base_addr: *mut ()) -> ResultCode {
    unsafe {
        dwc_otg_driver = DWC_OTG::init(base_addr);
    }

    // println!("| HCD: Initializing");

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

pub fn get_dwc_ptr(offset: usize) -> *mut u32 {
    unsafe { (dwc_otg_driver.base_addr + offset) as *mut u32 }
}

pub unsafe fn dwc_otg_initialize_controller(base_addr: *mut ()) {
    unsafe {
        dwc_otg_driver = DWC_OTG::init(base_addr);
    }
}

#[derive(Default)]
pub struct DwcChannelActive {
    pub channel: [u8; ChannelCount],
}

impl DwcChannelActive {
    pub const fn new() -> Self {
        Self {
            channel: [0; ChannelCount],
        }
    }
}

pub fn dwc_otg_get_active_channel() -> u8 {
    let mut dwc_channels = DWC_CHANNEL_ACTIVE.lock();
    for i in 1..ChannelCount {
        if dwc_channels.channel[i] == 0 {
            dwc_channels.channel[i] = 1;
            return i as u8;
        }
    }
    return ChannelCount as u8;
}

//TODO: Make this a mutex
pub fn dwc_otg_get_control_channel() -> u32 {
    let mut dwc_channels = DWC_CHANNEL_ACTIVE.lock();
    if dwc_channels.channel[0] == 0 {
        dwc_channels.channel[0] = 1;
        return 0 as u32;
    }
    return ChannelCount as u32;
}

pub fn dwc_otg_free_channel(channel: u32) {
    let mut dwc_channels = DWC_CHANNEL_ACTIVE.lock();
    dwc_channels.channel[channel as usize] = 0;
}

pub struct DwcChannelCallback {
    pub callback: [Option<fn(endpoint_descriptor, u32, u8)>; ChannelCount],
    pub endpoint_descriptors: [Option<endpoint_descriptor>; ChannelCount],
}

impl DwcChannelCallback {
    pub const fn new() -> Self {
        Self {
            callback: [None; ChannelCount],
            endpoint_descriptors: [None; ChannelCount],
        }
    }
}

pub struct DwcLock {}

impl DwcLock {
    pub const fn new() -> Self {
        Self {}
    }
}

pub struct DWC_OTG {
    base_addr: usize,
}

impl DWC_OTG {
    pub fn init(base_addr: *mut ()) -> Self {
        Self {
            base_addr: base_addr as usize,
        }
    }
}

pub struct dwc_hub {
    pub databuffer: [u8; 1024],
    pub phy_initialised: bool,
    pub dma_loc: usize,
    pub dma_phys: [usize; ChannelCount],
    pub dma_addr: [usize; ChannelCount],
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
    pub OddFrame: bool,
    pub Disable: bool,
    pub Enable: bool,
}

pub fn convert_into_host_characteristics(val: u32, chan: &mut host_characteristics) {
    chan.MaximumPacketSize = (val & HCCHAR_MPS_MASK) as u16;
    chan.EndPointNumber = ((val & HCCHAR_EPNUM_MASK) >> 11) as u8;
    chan.EndPointDirection = UsbDirection::from_u8(((val >> 15) & 0x1) as u8);
    chan.LowSpeed = ((val >> 17) & 0x1) != 0;
    chan.Type = UsbTransfer::from_u8(((val >> 18) & 0x3) as u8);
    chan.PacketsPerFrame = ((val & HCCHAR_MC_MASK) >> 20) as u8;
    chan.DeviceAddress = ((val & HCCHAR_DEVADDR_MASK) >> 22) as u8;
    chan.OddFrame = ((val >> 29) & 0x1) != 0;
    chan.Disable = ((val >> 30) & 0x1) != 0;
    chan.Enable = ((val >> 31) & 0x1) != 0;
}

pub fn convert_host_characteristics(chan: host_characteristics) -> u32 {
    return (chan.MaximumPacketSize as u32)
        | (chan.EndPointNumber as u32) << 11
        | (chan.EndPointDirection as u32) << 15
        | (chan.LowSpeed as u32) << 17
        | (chan.Type as u32) << 18
        | (chan.PacketsPerFrame as u32) << 20
        | (chan.DeviceAddress as u32) << 22
        | (chan.OddFrame as u32) << 29
        | (chan.Disable as u32) << 30
        | (chan.Enable as u32) << 31;
}

#[derive(Default, Copy, Clone)]
pub struct host_split_control {
    pub HubAddress: u32,
    pub PortAddress: u8,
    pub XactPos: u8,
    pub CompleteSplit: bool,
    pub SplitEnable: bool,
}

pub fn convert_into_host_split_control(val: u32, chan: &mut host_split_control) {
    chan.PortAddress = ((val >> 0) & HCSPLT_PRTADDR_MASK) as u8;
    chan.HubAddress = ((val & HCSPLT_HUBADDR_MASK) >> 7) as u32;
    chan.XactPos = ((val & HCSPLT_XACTPOS_MASK) >> 14) as u8;
    chan.CompleteSplit = ((val >> 16) & 0x1) != 0;
    chan.SplitEnable = ((val >> 31) & 0x1) != 0;
}

pub fn convert_host_split_control(chan: host_split_control) -> u32 {
    return (chan.PortAddress as u32) << 0
        | (chan.HubAddress as u32) << 7
        | (chan.XactPos as u32) << 14
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
    chan.PacketCount = (val >> 19) & 0x3ff;
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
            dma_loc: 0,
            dma_phys: [0; ChannelCount],
            dma_addr: [0; ChannelCount],
            channel: [host_channel::default(); ChannelCount],
        }
    }
}

#[repr(u8)]
#[derive(Default, Copy, Clone)]
pub enum PacketId {
    #[default]
    Data0 = 0,
    Data1 = 2,
    Data2 = 1,
    Setup = 3,
}

impl PacketId {
    pub fn from_u8(val: u8) -> PacketId {
        match val {
            0 => PacketId::Data0,
            2 => PacketId::Data1,
            1 => PacketId::Data2,
            3 => PacketId::Setup,
            _ => PacketId::Data0,
        }
    }
}

#[allow(dead_code)]
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
