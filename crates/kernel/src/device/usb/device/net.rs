/**
 *
 * usb/device/net.rs
 *  By Aaron Lo
 *   
 */
use super::super::usbd::descriptors::*;
use super::super::usbd::device::*;

use crate::device::system_timer;

use crate::networking::repr::*;
use crate::networking::utils::arp_cache::ArpCache;
use crate::networking::iface::Interface;
use crate::networking::iface::ethernet::recv_frame as recv_ethernet;

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

    // 1. new network interface
    // 2. initialize mac address
    // 3. initalize and statically set ip address and gateway

    // TODO: create dummy dhcp server instead of statically setting address
    // TODO: discussion with aaron about format for user, with NetSend/Recv or with my
    // interfce/device system i have set up. as it stands, we are going with the current method
    // with NetSend/Recv, so Interface is more like just acts like an enum of objects for the stack
    // to work on which is fine. will need to cutdown network stack into just parsing logic 
    // so many compiler warnings TT
    let DEFAULT_MAC = EthernetAddress::from_u32(OID::OID_802_3_PERMANENT_ADDRESS as u32);
    let DEFAULT_IPV4 = Ipv4Address::new([192, 168, 1, 1]); // tell aaron about cidr conventions
    let DEFAULT_IPV4CIDR = Ipv4Cidr::new(DEFAULT_IPV4, 24);
    let DEFAULT_GATEWAY = Ipv4Address::new([192, 168, 1, 1]); // change ts bruh

    // TODO: needs to be accessible from the NetDevice struct but limited lifetime issue
    // requires to change other references as well
    let default_interface = Interface {
        // dev: dummy_dev(),
        arp_cache: ArpCache::new(60, system_timer::get_time()),
        ethernet_addr: DEFAULT_MAC,
        ipv4_addr: DEFAULT_IPV4CIDR,
        default_gateway: DEFAULT_GATEWAY,
    };
    // initialize arp table
    
    // register ethernet receieve
    // RegisterNetReceiveCallback(recv_ethernet);
    

    // // Ethernet Frame
    // let dest_mac = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]; // Broadcast
    // let src_mac = [0x11, 0x11, 0x22, 0x33, 0x44, 0x55]; // Example MAC
    // let ethertype = [0x08, 0x00]; // IPv4
    //
    // // Write Ethernet Header
    // buffer[..6].copy_from_slice(&dest_mac);
    // buffer[6..12].copy_from_slice(&src_mac);
    // buffer[12..14].copy_from_slice(&ethertype);
    //
    // // Payload (minimal IP header for demonstration)
    // buffer[14..18].copy_from_slice(&[0x45, 0x00, 0x00, 0x28]); // IPv4 Header
    // buffer[18..22].copy_from_slice(&[0x00, 0x00, 0x40, 0x00]); // Identification & Flags
    // buffer[22..26].copy_from_slice(&[0x40, 0x11, 0xB7, 0xC8]); // TTL, Protocol (UDP), Checksum
    // buffer[26..30].copy_from_slice(&[192, 168, 1, 1]); // Source IP: 192.168.1.1
    // buffer[30..34].copy_from_slice(&[192, 168, 1, 2]); // Destination IP: 192.168.1.2
    //
    // // UDP Header (Example)
    // buffer[34..36].copy_from_slice(&[0x1F, 0x90]); // Source Port (8080)
    // buffer[36..38].copy_from_slice(&[0x00, 0x35]); // Destination Port (53 - DNS)
    // buffer[38..40].copy_from_slice(&[0x00, 0x14]); // Length (20 bytes)
    // buffer[40..42].copy_from_slice(&[0x00, 0x00]); // Checksum (assumed 0 for simplicity)
    //
    // // UDP Data (Example Payload)
    // buffer[42..46].copy_from_slice(b"PING");
    // println!("Ethernet Frame: {:02X?}", &buffer[..46]);
    
    // TODO: make from u32 so we can take in OID_802_3_PERMANENT_ADDRESS
    let arp_packet = ArpPacket {
        op: ArpOperation::Request,
        source_hw_addr: DEFAULT_MAC,
        source_proto_addr: DEFAULT_IPV4,
        target_hw_addr: EthernetAddress::BROADCAST,
        target_proto_addr: Ipv4Address::new([192, 168, 1, 2]),
    };

    println!("[+] sending test packet");

    let eth_buffer = [0u8; 42];
    let mut eth_frame = EthernetFrame::try_new(eth_buffer).unwrap();
    eth_frame.set_src_addr(DEFAULT_MAC);
    eth_frame.set_dst_addr(EthernetAddress::BROADCAST);
    eth_frame.set_payload_type(EthernetType::ARP);
    arp_packet.serialize(eth_frame.payload_mut()).unwrap();

    unsafe {
        NET_DEVICE.device = Some(device);
    }

    unsafe { // :skull:
        NetSendPacket(eth_frame.as_mut().as_mut_ptr(), 60);
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
            // callback(NET_DEVICE.default_interface, core::slice::from_raw_parts(buffer, buffer_length as usize), buffer_length); // fix this lmaooo
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
    // pub receive_callback: Option<fn(&mut Interface, &[u8], u32)>,
    pub device: Option<*mut UsbDevice>,
    // pub default_interface: &mut Interface,
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
