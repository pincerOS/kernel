

use super::super::configuration::*;
use super::super::types::*;
use super::descriptors::*;
use super::device::*;
use super::pipe::*;
use super::request::*;

use crate::device::system_timer::*;
use crate::shutdown;
use alloc::boxed::Box;
use crate::device::usb::UsbInterruptMessage;
use crate::device::usb::PacketId;

pub fn interrupt_endpoint_callback(endpoint: endpoint_descriptor) {
    let mut device = unsafe { &mut *endpoint.device };
    let mut pipe = UsbPipeAddress {
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
    let channel = 1;
    let result = UsbInterruptMessage(device, channel, pipe, buffer.as_mut_ptr(), 8, PacketId::Data0, endpoint.timeout);

    if result != ResultCode::OK {
        print!("| USB: Failed to read interrupt endpoint.\n");
    }

    // for i in 0..8 {
    //     print!("{:02X} ", buffer[i]);
    // }
    // print!("\n");
    let mut endpoint_device = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut UsbEndpointDevice) };

    if let Some(callback) = endpoint_device.endpoints[endpoint.device_endpoint_number as usize] {
        callback(buffer.as_mut_ptr());
    } else {
        println!("| USB: No callback for endpoint number {}.", endpoint.device_endpoint_number);
        shutdown();
    }
}

pub fn register_interrupt_endpoint(device: &mut UsbDevice, endpoint_time: u32, endpoint_address: u8, endpoint_direction: UsbDirection, endpoint_max_size: UsbPacketSize, device_endpoint_number: u8, timeout: u32) {
    let mut endpoint = endpoint_descriptor {
        endpoint_address: endpoint_address as u8,
        endpoint_direction: endpoint_direction,
        endpoint_type: UsbTransfer::Interrupt,
        max_packet_size: endpoint_max_size,
        device_endpoint_number: device_endpoint_number,
        device: device,
        device_number: device.number,
        device_speed: device.speed,
        buffer_length: 8,
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
    pub timeout: u32,
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
    pub endpoints: [Option<fn(*mut u8)>; 5],
}