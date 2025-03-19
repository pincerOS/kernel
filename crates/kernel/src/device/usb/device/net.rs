
use super::super::usbd::device::*;
use super::super::usbd::descriptors::*;
use super::super::usbd::usbd::*;

use alloc::vec;
use alloc::boxed::Box;
use crate::device::system_timer::micro_delay;
use crate::device::usb::hcd::dwc::dwc_otg::read_volatile;
use crate::device::usb::hcd::dwc::dwc_otgreg::*;
use crate::device::usb::types::*;
use crate::device::usb::hcd::hub::*;
use crate::device::usb::device::hid::keyboard::*;
use crate::device::usb::usbd::endpoint::register_interrupt_endpoint;
use crate::device::usb::usbd::endpoint::*;
use crate::device::usb::usbd::pipe::*;
use crate::device::usb::usbd::request::*;
use crate::device::usb::hcd::dwc::roothub::*;
use crate::device::usb::*;
use crate::shutdown;

use super::rndis::*;

pub fn NetLoad(bus: &mut UsbBus) {
    bus.interface_class_attach[InterfaceClass::InterfaceClassCommunications as usize] = Some(NetAttach);
}


pub fn NetAttach(device: &mut UsbDevice, interface_number: u32) -> ResultCode {

    println!("| Net: Subclass: {:x}, Protocol: {:x}", device.interfaces[interface_number as usize].subclass, device.interfaces[interface_number as usize].protocol);
    
    rndis_initialize_msg(device);

    let mut buffer = [0u8; 52];
    rndis_query_msg(device, OID::OID_GEN_CURRENT_PACKET_FILTER, buffer.as_mut_ptr(), 30);

    rndis_set_msg(device, OID::OID_GEN_CURRENT_PACKET_FILTER, 0xB);

    rndis_query_msg(device, OID::OID_GEN_CURRENT_PACKET_FILTER, buffer.as_mut_ptr(), 30);
    // shutdown();
    let boxed = Box::new(UsbEndpointDevice::new());
    let boxed_bytes = Box::into_raw(boxed);
    let byte_slice = unsafe { core::slice::from_raw_parts_mut(boxed_bytes as *mut u8, size_of::<UsbEndpointDevice>()) };
    let byte_bytes = unsafe { Box::from_raw(byte_slice as *mut [u8]) };
    //TODO: I have no clue what I'm doing
    device.driver_data = Some(byte_bytes);

    let mut endpoint_device = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut UsbEndpointDevice) };
    endpoint_device.endpoints[0] = Some(NetAnalyze);


    // println!("Device interface number: {:?}", device.interface_number);
    println!("Device interface number: {:?}", device.endpoints[interface_number as usize][0 as usize].endpoint_address);
    println!("Device endpoint interval: {:?}", device.endpoints[interface_number as usize][0 as usize].interval);    

    // register_interrupt_endpoint(
    //     device,
    //     device.endpoints[interface_number as usize][0 as usize].interval as u32, 
    //     endpoint_address_to_num(device.endpoints[interface_number as usize][0 as usize].endpoint_address), 
    //     UsbDirection::In, 
    //     size_from_number(device.endpoints[interface_number as usize][0 as usize].packet.MaxSize as u32),
    //     0,
    //     10
    // );

    let mut buffer = [0u8; 64];
    // Ethernet Frame
    let dest_mac = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]; // Broadcast
    let src_mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]; // Example MAC
    let ethertype = [0x08, 0x00]; // IPv4

    // Write Ethernet Header
    buffer[..6].copy_from_slice(&dest_mac);
    buffer[6..12].copy_from_slice(&src_mac);
    buffer[12..14].copy_from_slice(&ethertype);

    // Payload (minimal IP header for demonstration)
    buffer[14..18].copy_from_slice(&[0x45, 0x00, 0x00, 0x28]); // IPv4 Header
    buffer[18..22].copy_from_slice(&[0x00, 0x00, 0x40, 0x00]); // Identification & Flags
    buffer[22..26].copy_from_slice(&[0x40, 0x11, 0xB7, 0xC8]); // TTL, Protocol (UDP), Checksum
    buffer[26..30].copy_from_slice(&[192, 168, 1, 1]);          // Source IP: 192.168.1.1
    buffer[30..34].copy_from_slice(&[192, 168, 1, 2]);          // Destination IP: 192.168.1.2


    // UDP Header (Example)
    buffer[34..36].copy_from_slice(&[0x1F, 0x90]); // Source Port (8080)
    buffer[36..38].copy_from_slice(&[0x00, 0x35]); // Destination Port (53 - DNS)
    buffer[38..40].copy_from_slice(&[0x00, 0x14]); // Length (20 bytes)
    buffer[40..42].copy_from_slice(&[0x00, 0x00]); // Checksum (assumed 0 for simplicity)

    // UDP Data (Example Payload)
    buffer[42..46].copy_from_slice(b"PING");

    println!("Ethernet Frame: {:02X?}", &buffer[..46]);

    // rndis_send_packet(device, buffer.as_mut_ptr(), 64);
    micro_delay(1000000);
    rndis_receive_packet(device, buffer.as_mut_ptr(), 64);
    shutdown();
    return ResultCode::OK;
}

pub fn NetAnalyze(buffer: *mut u8) {
    println!("| Net: Analyzing buffer");
    let buffer32 = unsafe { core::slice::from_raw_parts(buffer, 32) };
    println!("Buffer 0 {:?}", buffer32[0]);
    println!("Buffer 1 {:?}", buffer32[1]);

    let buffer_ptr32 = buffer as *mut u32;
    println!("buffer_ptr32 {:?}", unsafe { *buffer_ptr32 });
    println!("buffer_ptr32 {:?}", unsafe { *buffer_ptr32.offset(1) });

    if buffer32[0] != 0 {
        println!("| Net: Buffer1: {:?}", buffer32[0]);
        shutdown();
    }

    if unsafe { *buffer_ptr32 } != 0 {
        println!("| Net: Buffer2: {:?}", unsafe { *buffer_ptr32 });
        shutdown();
    }
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

pub const fn convert_usb_device_request_cdc(request: UsbDeviceRequestCDC) -> UsbDeviceRequestRequest {
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