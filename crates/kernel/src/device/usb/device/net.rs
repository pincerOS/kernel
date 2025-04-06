/**
 *
 * usb/device/net.rs
 *  By Aaron Lo
 *   
 */
use super::super::usbd::descriptors::*;
use super::super::usbd::device::*;

use crate::device::usb::types::*;
use crate::device::usb::usbd::endpoint::register_interrupt_endpoint;
use crate::device::usb::usbd::endpoint::*;
use crate::device::usb::usbd::request::*;
use crate::shutdown;
use alloc::boxed::Box;

use super::rndis::*;

pub static mut NET_DEVICE: NetDevice = NetDevice {
    receive_callback: None,
    device: None,
};

pub fn NetLoad(bus: &mut UsbBus) {
    bus.interface_class_attach[InterfaceClass::InterfaceClassCommunications as usize] =
        Some(NetAttach);
}

pub fn NetAttach(device: &mut UsbDevice, interface_number: u32) -> ResultCode {
    // println!(
    //     "| Net: Subclass: {:x}, Protocol: {:x}",
    //     device.interfaces[interface_number as usize].subclass,
    //     device.interfaces[interface_number as usize].protocol
    // );
    println!("| Net: Usb Hub Detected");
    rndis_initialize_msg(device);

    let mut buffer = [0u8; 52];

    unsafe {
        rndis_query_msg(
            device,
            OID::OID_GEN_CURRENT_PACKET_FILTER,
            buffer.as_mut_ptr(),
            30,
        );

        rndis_set_msg(device, OID::OID_GEN_CURRENT_PACKET_FILTER, 0xB);

        rndis_query_msg(
            device,
            OID::OID_GEN_CURRENT_PACKET_FILTER,
            buffer.as_mut_ptr(),
            30,
        );
    }

    let driver_data = Box::new(UsbEndpointDevice::new());
    device.driver_data = DriverData::new(driver_data);

    let endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();
    endpoint_device.endpoints[0] = Some(NetAnalyze);
    endpoint_device.endpoints[1] = Some(NetSend);
    endpoint_device.endpoints[2] = Some(NetReceive);

    // println!(
    //     "Device interface number: {:?}",
    //     device.endpoints[interface_number as usize][0 as usize].endpoint_address
    // );
    // println!(
    //     "Device endpoint interval: {:?}",
    //     device.endpoints[interface_number as usize][0 as usize].interval
    // );

    register_interrupt_endpoint(
        device,
        device.endpoints[interface_number as usize][0 as usize].interval as u32,
        endpoint_address_to_num(
            device.endpoints[interface_number as usize][0 as usize].endpoint_address,
        ),
        UsbDirection::In,
        size_from_number(
            device.endpoints[interface_number as usize][0 as usize]
                .packet
                .MaxSize as u32,
        ),
        0,
        10,
    );

    let mut buffer = [0u8; 64];
    // Ethernet Frame
    let dest_mac = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]; // Broadcast
    let src_mac = [0x11, 0x11, 0x22, 0x33, 0x44, 0x55]; // Example MAC
    let ethertype = [0x08, 0x00]; // IPv4

    // Write Ethernet Header
    buffer[..6].copy_from_slice(&dest_mac);
    buffer[6..12].copy_from_slice(&src_mac);
    buffer[12..14].copy_from_slice(&ethertype);

    // Payload (minimal IP header for demonstration)
    buffer[14..18].copy_from_slice(&[0x45, 0x00, 0x00, 0x28]); // IPv4 Header
    buffer[18..22].copy_from_slice(&[0x00, 0x00, 0x40, 0x00]); // Identification & Flags
    buffer[22..26].copy_from_slice(&[0x40, 0x11, 0xB7, 0xC8]); // TTL, Protocol (UDP), Checksum
    buffer[26..30].copy_from_slice(&[192, 168, 1, 1]); // Source IP: 192.168.1.1
    buffer[30..34].copy_from_slice(&[192, 168, 1, 2]); // Destination IP: 192.168.1.2

    // UDP Header (Example)
    buffer[34..36].copy_from_slice(&[0x1F, 0x90]); // Source Port (8080)
    buffer[36..38].copy_from_slice(&[0x00, 0x35]); // Destination Port (53 - DNS)
    buffer[38..40].copy_from_slice(&[0x00, 0x14]); // Length (20 bytes)
    buffer[40..42].copy_from_slice(&[0x00, 0x00]); // Checksum (assumed 0 for simplicity)

    // UDP Data (Example Payload)
    buffer[42..46].copy_from_slice(b"PING");

    // println!("Ethernet Frame: {:02X?}", &buffer[..46]);

    unsafe {
        NET_DEVICE.device = Some(device);
    }
    // unsafe {
    // rndis_send_packet(device, buffer.as_mut_ptr(), 64);
    // rndis_receive_packet(device, Box::new(buffer), 64);
    // }
    // micro_delay(1000000);
    // shutdown();
    return ResultCode::OK;
}

pub unsafe fn NetAnalyze(buffer: *mut u8, buffer_length: u32) {
    let buffer32 = unsafe { core::slice::from_raw_parts(buffer, buffer_length as usize) };
    if buffer32.is_empty() {
        return;
    }

    if buffer32[0] != 0 {
        println!("| Net: Buffer1: {:?}", buffer32[0]);
        shutdown();
    }
}

pub fn NetSend(_buffer: *mut u8, _buffer_length: u32) {
    //Do nothing for now
    //Called when USB packet is actually sent out
}

pub fn NetReceive(buffer: *mut u8, buffer_length: u32) {
    println!("| Net: Receive");

    unsafe {
        if let Some(callback) = NET_DEVICE.receive_callback {
            callback(buffer, buffer_length);
        } else {
            println!("| Net: No callback for receive.");
        }
    }
}

pub fn RegisterNetReceiveCallback(callback: fn(*mut u8, u32)) {
    unsafe {
        NET_DEVICE.receive_callback = Some(callback);
    }
}

pub unsafe fn NetSendPacket(buffer: *mut u8, buffer_length: u32) {
    unsafe {
        if let Some(device) = NET_DEVICE.device {
            let usb_dev = &mut *device;
            rndis_send_packet(usb_dev, buffer, buffer_length);
        } else {
            println!("| Net: No device found.");
            shutdown();
        }
    }
}

pub struct NetDevice {
    pub receive_callback: Option<fn(*mut u8, u32)>,
    pub device: Option<*mut UsbDevice>,
}

#[repr(u32)]
#[derive(Default, Debug, Clone, Copy)]
pub enum UsbDeviceRequestCDC {
    #[default]
    SendEncapsulatedCommand = 0x00,
    GetEncapsulatedResponse = 0x01,
    SetCommFeature = 0x02,
    GetCommFeature = 0x03,
    SetLineCoding = 0x20,
    GetLineCoding = 0x21,
    SetControlLineState = 0x22,
    SendBreak = 0x23,
}

pub const fn convert_usb_device_request_cdc(
    request: UsbDeviceRequestCDC,
) -> UsbDeviceRequestRequest {
    match request {
        UsbDeviceRequestCDC::SendEncapsulatedCommand => UsbDeviceRequestRequest::GetStatus,
        UsbDeviceRequestCDC::GetEncapsulatedResponse => UsbDeviceRequestRequest::ClearFeature,
        UsbDeviceRequestCDC::SetCommFeature => UsbDeviceRequestRequest::GetIdle,
        UsbDeviceRequestCDC::GetCommFeature => UsbDeviceRequestRequest::SetFeature,
        UsbDeviceRequestCDC::SetLineCoding => UsbDeviceRequestRequest::SetLineCoding,
        UsbDeviceRequestCDC::GetLineCoding => UsbDeviceRequestRequest::GetLineCoding,
        UsbDeviceRequestCDC::SetControlLineState => UsbDeviceRequestRequest::SetControlLineState,
        UsbDeviceRequestCDC::SendBreak => UsbDeviceRequestRequest::SendBreak,
    }
}
