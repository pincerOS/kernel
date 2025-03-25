/**
 *
 * usbd/endpoint.rs
 *  By Aaron Lo
 *   
 *   This file contains implemenation for USB endpoints
 *
 */
use super::super::types::*;
use super::device::*;
use super::pipe::*;

use crate::device::system_timer::*;
use crate::device::usb::dwc_hub;
use crate::device::usb::hcd::dwc::dwc_otg::HcdUpdateTransferSize;
use crate::device::usb::hcd::dwc::dwc_otg::DWC_CHANNEL_CALLBACK;
use crate::device::usb::hcd::dwc::dwc_otgreg::HCINT_CHHLTD;
use crate::device::usb::hcd::dwc::dwc_otgreg::HCINT_NAK;
use crate::device::usb::hcd::dwc::dwc_otgreg::HCINT_XFERCOMPL;
use crate::device::usb::PacketId;
use crate::device::usb::UsbInterruptMessage;
use crate::shutdown;
use alloc::boxed::Box;

pub fn finish_bulk_endpoint_callback_in(endpoint: endpoint_descriptor, hcint: u32) {
    let device = unsafe { &mut *endpoint.device };

    let transfer_size = HcdUpdateTransferSize(device, endpoint.channel);
    device.last_transfer = endpoint.buffer_length - transfer_size;
    let endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();

    if hcint & HCINT_CHHLTD == 0 {
        println!(
            "| Endpoint {} in: HCINT_CHHLTD not set, aborting. hcint: {:x}.",
            endpoint.channel, hcint
        );
        shutdown();
    }

    if hcint & HCINT_XFERCOMPL == 0 {
        println!(
            "| Endpoint {} in: HCINT_XFERCOMPL not set, aborting. {:x}",
            endpoint.channel, hcint
        );
        shutdown();
    }

    let dwc_sc = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };
    let dma_addr = dwc_sc.dma_addr[endpoint.channel as usize];

    let buffer = endpoint.buffer;

    unsafe {
        core::ptr::copy_nonoverlapping(dma_addr as *const u8, buffer, 8);
    }

    if let Some(callback) = endpoint_device.endpoints[endpoint.device_endpoint_number as usize] {
        unsafe { callback(buffer, device.last_transfer) };
    } else {
        println!(
            "| USB: No callback for endpoint number {}.",
            endpoint.device_endpoint_number
        );
        shutdown();
    }
}

pub fn finish_bulk_endpoint_callback_out(endpoint: endpoint_descriptor, hcint: u32) {
    let device = unsafe { &mut *endpoint.device };
    let transfer_size = HcdUpdateTransferSize(device, endpoint.channel);
    device.last_transfer = endpoint.buffer_length - transfer_size;

    if hcint & HCINT_CHHLTD == 0 {
        println!(
            "| Endpoint {}: HCINT_CHHLTD not set, aborting.",
            endpoint.channel
        );
        shutdown();
    }

    if hcint & HCINT_XFERCOMPL == 0 {
        println!(
            "| Endpoint {}: HCINT_XFERCOMPL not set, aborting.",
            endpoint.channel
        );
        shutdown();
    }

    //Good to go
}

pub fn finish_interrupt_endpoint_callback(endpoint: endpoint_descriptor, hcint: u32) {
    let device = unsafe { &mut *endpoint.device };
    let transfer_size = HcdUpdateTransferSize(device, endpoint.channel);
    device.last_transfer = endpoint.buffer_length - transfer_size;
    let endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();

    //TODO: Hardcoded for usb-kbd for now
    let mut buffer = Box::new([0u8; 8]);
    let dwc_sc = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };

    let dma_addr = dwc_sc.dma_addr[endpoint.channel as usize];

    if hcint & HCINT_CHHLTD == 0 {
        println!(
            "| Endpoint {}: HCINT_CHHLTD not set, aborting. hcint: {:x}.",
            endpoint.channel, hcint
        );
        shutdown();
    }

    if hcint & HCINT_NAK != 0 {
        //NAK received, do nothing
    } else if hcint & HCINT_XFERCOMPL != 0 {
        //Transfer complete
        //copy from dma_addr to buffer
        unsafe {
            core::ptr::copy_nonoverlapping(dma_addr as *const u8, buffer.as_mut_ptr(), 8);
        }
    } else {
        println!(
            "| Endpoint {}: Unknown interrupt, aborting.",
            endpoint.channel
        );
        shutdown();
    }

    if let Some(callback) = endpoint_device.endpoints[endpoint.device_endpoint_number as usize] {
        unsafe { callback(buffer.as_mut_ptr(), device.last_transfer) };
    } else {
        println!(
            "| USB: No callback for endpoint number {}.",
            endpoint.device_endpoint_number
        );
        shutdown();
    }
}

pub fn interrupt_endpoint_callback(endpoint: endpoint_descriptor) {
    let device = unsafe { &mut *endpoint.device };
    let pipe = UsbPipeAddress {
        transfer_type: UsbTransfer::Interrupt,
        speed: device.speed,
        end_point: endpoint.endpoint_address,
        device: device.number as u8,
        direction: endpoint.endpoint_direction,
        max_size: endpoint.max_packet_size,
        _reserved: 0,
    };

    unsafe {
        DWC_CHANNEL_CALLBACK.callback[endpoint.channel as usize] =
            Some(finish_interrupt_endpoint_callback);
        DWC_CHANNEL_CALLBACK.endpoint_descriptors[endpoint.channel as usize] = Some(endpoint);
    }

    //TODO: Hardcoded for usb-kbd for now
    let mut buffer = Box::new([0u8; 8]);
    let channel = endpoint.channel;
    let result = unsafe {
        UsbInterruptMessage(
            device,
            channel,
            pipe,
            buffer.as_mut_ptr(),
            8,
            PacketId::Data0,
            endpoint.timeout,
        )
    };

    if result != ResultCode::OK {
        print!("| USB: Failed to read interrupt endpoint.\n");
    }

    // for i in 0..8 {
    //     print!("{:02X} ", buffer[i]);
    // }
    // print!("\n");
    // let mut endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();

    // if let Some(callback) = endpoint_device.endpoints[endpoint.device_endpoint_number as usize] {
    //     callback(buffer.as_mut_ptr());
    // } else {
    //     println!("| USB: No callback for endpoint number {}.", endpoint.device_endpoint_number);
    //     shutdown();
    // }
}

pub fn register_interrupt_endpoint(
    device: &mut UsbDevice,
    channel: u8,
    endpoint_time: u32,
    endpoint_address: u8,
    endpoint_direction: UsbDirection,
    endpoint_max_size: UsbPacketSize,
    device_endpoint_number: u8,
    timeout: u32,
) {
    let endpoint = endpoint_descriptor {
        endpoint_address: endpoint_address as u8,
        endpoint_direction: endpoint_direction,
        endpoint_type: UsbTransfer::Interrupt,
        max_packet_size: endpoint_max_size,
        device_endpoint_number: device_endpoint_number,
        device: device,
        device_number: device.number,
        device_speed: device.speed,
        buffer_length: 8,
        buffer: core::ptr::null_mut(),
        channel: channel,
        timeout: timeout,
    };

    timer_scheduler_add_timer_event(endpoint_time, interrupt_endpoint_callback, endpoint);
}

#[derive(Copy, Clone)]
pub struct endpoint_descriptor {
    pub endpoint_address: u8,
    pub endpoint_direction: UsbDirection,
    pub endpoint_type: UsbTransfer,
    pub max_packet_size: UsbPacketSize,
    pub device_endpoint_number: u8,
    pub device: *mut UsbDevice,
    pub device_number: u32,
    pub device_speed: UsbSpeed,
    pub buffer_length: u32,
    pub buffer: *mut u8,
    pub channel: u8,
    pub timeout: u32,
}

impl endpoint_descriptor {
    pub fn new() -> Self {
        endpoint_descriptor {
            endpoint_address: 0,
            endpoint_direction: UsbDirection::Out,
            endpoint_type: UsbTransfer::Control,
            max_packet_size: UsbPacketSize::Bits8,
            device_endpoint_number: 0,
            device: core::ptr::null_mut(),
            device_number: 0,
            device_speed: UsbSpeed::Low,
            buffer_length: 0,
            buffer: core::ptr::null_mut(),
            channel: 0,
            timeout: 0,
        }
    }
}

impl UsbEndpointDevice {
    pub fn new() -> Self {
        UsbEndpointDevice {
            endpoints: [None; 5],
        }
    }
}

pub struct UsbEndpointDevice {
    //TODO: update for better?: The 5 is an arbitrary number
    pub endpoints: [Option<unsafe fn(*mut u8, u32)>; 5],
}
