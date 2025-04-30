use super::super::usbd::descriptors::*;
use super::super::usbd::device::*;
/**
 *
 * usb/device/net.rs
 *  By Aaron Lo
 *   
 */
use core::slice;

use crate::networking::iface::*;
use crate::networking::repr::*;

use crate::device::usb::types::*;
use crate::device::usb::usbd::endpoint::register_interrupt_endpoint;
use crate::device::usb::usbd::endpoint::*;
use crate::device::usb::usbd::request::*;
use crate::shutdown;

use alloc::boxed::Box;
use alloc::vec;

use super::rndis::*;

pub static mut NET_DEVICE: NetDevice = NetDevice {
    receive_callback: None,
    device: None,
};

pub static mut INTERFACE: Option<Interface> = None;

#[allow(static_mut_refs)]
pub fn get_interface_mut() -> &'static mut Interface {
    unsafe { INTERFACE.as_mut().expect("INTERFACE not initialized") }
}

pub static mut DHCPD: Option<dhcp::Dhcpd> = None;

#[allow(static_mut_refs)]
pub fn get_dhcpd_mut() -> &'static mut dhcp::Dhcpd {
    unsafe { DHCPD.as_mut().expect("DHCPD not initialized") }
}
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

        rndis_set_msg(device, OID::OID_GEN_CURRENT_PACKET_FILTER, 0xF);

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
    // 4. initialize arp table

    let mac_addr: &mut [u8; 6];
    unsafe {
        let mut b = vec![0u8; 30];
        let query = rndis_query_msg(device, OID::OID_802_3_PERMANENT_ADDRESS, b.as_mut_ptr(), 30);

        if query.0 != ResultCode::OK {
            panic!("| Net: Error getting MAC address {:#?}", query.0);
        }

        let b_offset = query.1;
        let b_len = query.2;
        if b_len != 6 {
            panic!("| Net: Error getting MAC address {}", b_len);
        }
        mac_addr = &mut *(b.as_mut_ptr().offset(b_offset as isize) as *mut [u8; 6]);
    }

    println!("| Net: MAC Address: {:x?}", mac_addr);
    let DEFAULT_MAC = EthernetAddress::from_bytes(mac_addr).unwrap();

    // initalize the interface
    unsafe {
        INTERFACE = Some(Interface::new());
    }

    // set the mac address
    let interface = get_interface_mut();
    interface.ethernet_addr = DEFAULT_MAC;

    // register receiving ethernet function
    RegisterNetReceiveCallback(recv);

    unsafe {
        NET_DEVICE.device = Some(device);
    }

    // begin receieve series, this queues a receive to be ran which will eventually propogate back
    // to us through the rgistered `recv` function which then queues another receive
    let buf = vec![0u8; 1600];
    unsafe {
        rndis_receive_packet(device, buf.into_boxed_slice(), 1500); // TODO: ask aaron if I need to use another function?
    }

    // start dhcp
    unsafe {
        DHCPD = Some(dhcp::Dhcpd::new());
    }

    // begin socket send loop, this iterates through all existing sockets, and attempts to send as
    // many packets as possible from each socket
    // socket::socket_send_loop();

    return ResultCode::OK;
}

pub unsafe fn recv(buf: *mut u8, buf_len: u32) {
    // cast our buffer into a Vec<u8>
    let slice: &[u8] = unsafe { slice::from_raw_parts(buf, buf_len as usize) };

    let interface = get_interface_mut();
    let _ = ethernet::recv_ethernet_frame(interface, slice, buf_len);
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
    // Do nothing for now
    // Called when USB packet is actually sent out
    println!("NetSend called");
}

pub unsafe fn NetReceive(buffer: *mut u8, buffer_length: u32) {
    // println!("| Net: Receive");

    unsafe {
        if let Some(callback) = NET_DEVICE.receive_callback {
            // callback(NET_DEVICE.default_interface, core::slice::from_raw_parts(buffer, buffer_length as usize), buffer_length); // fix this lmaooo
            callback(buffer, buffer_length);
        } else {
            println!("| Net: No callback for receive.");
        }
    }

    let buf = vec![0u8; 1];
    unsafe {
        let device = &mut *NET_DEVICE.device.unwrap();
        rndis_receive_packet(device, buf.into_boxed_slice(), 1600);
    }
}

pub fn RegisterNetReceiveCallback(callback: unsafe fn(*mut u8, u32)) {
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
    pub receive_callback: Option<unsafe fn(*mut u8, u32)>,
    // pub receive_callback: Option<fn(&mut Interface, &[u8], u32)>,
    pub device: Option<*mut UsbDevice>,
    // pub default_interface: Option<Box<Interface>>,
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
