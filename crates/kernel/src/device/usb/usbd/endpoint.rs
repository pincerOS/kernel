/**
 *
 * usbd/endpoint.rs
 *  By Aaron Lo
 *   
 *   This file contains implemenation for USB endpoints
 *
 */
use crate::device::usb;

use crate::device::usb::UsbSendInterruptMessage;
use usb::dwc_hub;
use usb::hcd::dwc::dwc_otg::HcdUpdateTransferSize;
use usb::hcd::dwc::dwc_otgreg::{HCINT_CHHLTD, HCINT_NAK, HCINT_XFERCOMPL};
use usb::types::*;
use usb::usbd::device::*;
use usb::usbd::pipe::*;
use usb::PacketId;

use crate::event::task::spawn_async_rt;
use crate::shutdown;
use crate::sync::time::{interval, MissedTicks};

use alloc::boxed::Box;

pub fn finish_bulk_endpoint_callback_in(endpoint: endpoint_descriptor, hcint: u32, channel: u8) {
    let device = unsafe { &mut *endpoint.device };

    let transfer_size = HcdUpdateTransferSize(device, channel);
    device.last_transfer = endpoint.buffer_length - transfer_size;
    let endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();

    if hcint & HCINT_CHHLTD == 0 {
        println!(
            "| Endpoint {} in: HCINT_CHHLTD not set, aborting. hcint: {:x}.",
            channel, hcint
        );
        shutdown();
    }

    if hcint & HCINT_XFERCOMPL == 0 {
        println!(
            "| Endpoint {} in: HCINT_XFERCOMPL not set, aborting. {:x}",
            channel, hcint
        );
        shutdown();
    }

    let dwc_sc = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };
    let dma_addr = dwc_sc.dma_addr[channel as usize];

    let buffer = endpoint.buffer;
    let buffer_length = device.last_transfer;
    unsafe {
        core::ptr::copy_nonoverlapping(dma_addr as *const u8, buffer, buffer_length as usize);
    }

    if let Some(callback) = endpoint_device.endpoints[endpoint.device_endpoint_number as usize] {
        // TODO: make this take a slice
        unsafe { callback(buffer, device.last_transfer) };
    } else {
        println!(
            "| USB: No callback for endpoint number {}.",
            endpoint.device_endpoint_number
        );
        shutdown();
    }
}

pub fn finish_bulk_endpoint_callback_out(endpoint: endpoint_descriptor, hcint: u32, channel: u8) {
    let device = unsafe { &mut *endpoint.device };
    let transfer_size = HcdUpdateTransferSize(device, channel);
    device.last_transfer = endpoint.buffer_length - transfer_size;

    if hcint & HCINT_CHHLTD == 0 {
        println!("| Endpoint {}: HCINT_CHHLTD not set, aborting.", channel);
        shutdown();
    }

    if hcint & HCINT_XFERCOMPL == 0 {
        println!("| Endpoint {}: HCINT_XFERCOMPL not set, aborting.", channel);
        shutdown();
    }

    //Good to go
}

pub fn finish_interrupt_endpoint_callback(endpoint: endpoint_descriptor, hcint: u32, channel: u8) {
    let device = unsafe { &mut *endpoint.device };
    let transfer_size = HcdUpdateTransferSize(device, channel);
    device.last_transfer = endpoint.buffer_length - transfer_size;
    let endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();

    //TODO: Hardcoded for usb-kbd for now
    let dwc_sc = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };

    let dma_addr = dwc_sc.dma_addr[channel as usize];

    if hcint & HCINT_CHHLTD == 0 {
        println!(
            "| Endpoint {}: HCINT_CHHLTD not set, aborting. hcint: {:x}.",
            channel, hcint
        );
        shutdown();
    }

    let buffer_length = device.last_transfer.clamp(0, 8);
    let mut buffer = Box::new_uninit_slice(buffer_length as usize);

    if hcint & HCINT_NAK != 0 {
        //NAK received, do nothing
        assert_eq!(buffer_length, 0);
    } else if hcint & HCINT_XFERCOMPL != 0 {
        //Transfer complete
        //copy from dma_addr to buffer
        unsafe {
            core::ptr::copy_nonoverlapping(
                dma_addr as *const u8,
                buffer.as_mut_ptr().cast(),
                buffer_length as usize,
            );
        }
    } else {
        println!("| Endpoint {}: Unknown interrupt, aborting.", channel);
        shutdown();
    }

    let mut buffer = unsafe { buffer.assume_init() };

    if let Some(callback) = endpoint_device.endpoints[endpoint.device_endpoint_number as usize] {
        unsafe { callback(buffer.as_mut_ptr(), buffer_length) };
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

    //TODO: Hardcoded for usb-kbd for now
    let mut buffer = Box::new([0u8; 8]);
    let result = unsafe {
        UsbSendInterruptMessage(
            device,
            pipe,
            buffer.as_mut_ptr(),
            8,
            PacketId::Data0,
            endpoint.timeout,
            finish_interrupt_endpoint_callback,
            endpoint,
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
        timeout: timeout,
    };

    spawn_async_rt(async move {
        let μs = endpoint_time as u64 * 1000;
        let mut interval = interval(μs).with_missed_tick_behavior(MissedTicks::Skip);
        while interval.tick().await {
            interrupt_endpoint_callback(endpoint);
        }
    });
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
    pub timeout: u32,
}

unsafe impl Sync for endpoint_descriptor {}
unsafe impl Send for endpoint_descriptor {}

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
