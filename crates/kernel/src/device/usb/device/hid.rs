/******************************************************************************
*	device/hid/hid.c
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*	device/hid/hid.c contains code relating to the generic USB human interface 
*	device driver. Human interface devices have another standard on top of USB
*	(oh boy!) which is actually very neat. It allows human interface devices to
*	describe their buttons, sliders and dials in great detail, and allows a 
*	flexible driver to handle them all. This driver merely provides methods to
*	deal with these reports. More abstracted drivers for keyboards and mice and
*	whatnot would no doubt be very useful.
******************************************************************************/

pub mod keyboard;

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
use crate::device::usb::usbd::pipe::*;
use crate::device::usb::usbd::request::*;
use crate::device::usb::hcd::dwc::roothub::*;
use crate::device::usb::*;

pub const HidMessageTimeout:u32 = 10;

pub fn HidLoad(bus: &mut UsbBus) {
    bus.interface_class_attach[InterfaceClass::InterfaceClassHid as usize] = Some(HidAttach);
}

fn HidSetProtocol(device: &mut UsbDevice, interface: u8, protocol: u16) -> ResultCode {
    let result = UsbControlMessage(
        device,
        UsbPipeAddress {
            transfer_type: UsbTransfer::Control,
            speed: device.speed,
            end_point: 0,
            device: device.number as u8,
            direction: UsbDirection::Out,
            max_size: size_from_number(device.descriptor.max_packet_size0 as u32),
            _reserved: 0,
        },
        core::ptr::null_mut(),
        0,
        &mut UsbDeviceRequest {
            request_type: 0x21,
            request: UsbDeviceHidRequest::convert_request(UsbDeviceHidRequest::SetProtocol),
            index: interface as u16,
            value: protocol,
            length: 0,
        },
        HidMessageTimeout,
    );

    if result != ResultCode::OK {
        print!("| HID: Failed to set protocol.\n");
        return result;
    }

    return ResultCode::OK;
}

fn HidGetReport(device: &mut UsbDevice, report_type: HidReportType, report_id: u8, interface: u8, buffer_length: u32, buffer:  *mut u8) -> ResultCode {
    let result = UsbControlMessage(
        device, 
        UsbPipeAddress {
            transfer_type: UsbTransfer::Control,
            speed: device.speed,
            end_point: 0,
            device: device.number as u8,
            direction: UsbDirection::In,
            max_size: size_from_number(device.descriptor.max_packet_size0 as u32),
            _reserved: 0,
        }, 
        buffer, 
        buffer_length, 
        &mut UsbDeviceRequest {
            request_type: 0xa1,
            request: UsbDeviceHidRequest::convert_request(UsbDeviceHidRequest::GetReport),
            index: interface as u16,
            value: (report_type as u16) << 8 | report_id as u16,
            length: buffer_length as u16,
        }, 
        HidMessageTimeout
    );

    if result != ResultCode::OK {
        print!("| HID: Failed to get report.\n");
        return result;
    }

    return ResultCode::OK;
}


pub fn HidAttach(device: &mut UsbDevice, interface_number: u32) -> ResultCode {

    println!("| HID: Attaching to interface {}.", interface_number);
    let mut result;

    if device.interfaces[interface_number as usize].class != InterfaceClass::InterfaceClassHid {
        println!("HID: Interface {} is not a HID interface.", interface_number);
        return ResultCode::ErrorArgument;
    }

    if device.interfaces[interface_number as usize].endpoint_count < 1 {
        println!("HID: Interface {} has no endpoints.", interface_number);
        return ResultCode::ErrorArgument;
    }

    // if device.endpoints[interface_number as usize][0].endpoint_address

    //print Subclass and Protocol
    println!("| HID: Subclass: {:x}, Protocol: {:x}", device.interfaces[interface_number as usize].subclass, device.interfaces[interface_number as usize].protocol);

    println!("| HID information:\n{:#?}", device.interfaces[interface_number as usize]);

    for i in 0..device.interfaces[interface_number as usize].endpoint_count {
        println!("| HID: Endpoint {} information:\n{:#?}", i, device.endpoints[interface_number as usize][i as usize]);
    }

    //TODO: ignore for now
    // if (device->Endpoints[interfaceNumber][0].EndpointAddress.Direction != In ||
	// 	device->Endpoints[interfaceNumber][0].Attributes.Type != Interrupt) {
	// 	LOG("HID: Invalid HID device with unusual endpoints (0).\n");
	// 	return ErrorIncompatible;
	// }
	// if (device->Interfaces[interfaceNumber].EndpointCount >= 2) {
	// 	if (device->Endpoints[interfaceNumber][1].EndpointAddress.Direction != Out ||
	// 		device->Endpoints[interfaceNumber][1].Attributes.Type != Interrupt) {
	// 		LOG("HID: Invalid HID device with unusual endpoints (1).\n");
	// 		return ErrorIncompatible;
	// 	}	
	// }

    if device.status != UsbDeviceStatus::Configured {
        println!("| HID: Device is not configured.");
        return ResultCode::ErrorDevice;
    }

    if device.interfaces[interface_number as usize].subclass == 0x01 {
        if device.interfaces[interface_number as usize].protocol == 0x01 {
            println!("| HID: Keyboard detected.");
        }
        else {
            println!("| HID: Unknown HID device detected: {:#x}", device.interfaces[interface_number as usize].protocol);
            return ResultCode::ErrorArgument;
        }

        result = HidSetProtocol(device, interface_number as u8, 1);
        if result != ResultCode::OK {
            println!("| HID: Could not revert to report mode from HID mode");
            return result;
        }
    }   


    let header = device.full_configuration.as_mut().unwrap().as_mut_ptr() as *mut UsbDescriptorHeader;
    let mut buffer = Box::new([0u8; 10]);
    //https://github.com/tmk/tmk_keyboard/wiki/USB%3A-HID-Usage-Table
    
    
    let result = HidGetReport(device, HidReportType::Feature, 0, interface_number as u8, 8, buffer.as_mut_ptr());
    println!("| HID: Get report result: {:?}", result);
    //print 8 bytes of buffer
    for i in 0..8 {
        print!("{:x} ", buffer[i]);
    }
    println!();

    micro_delay(500000);

    println!("| HID: endpoint address: {:x}", endpoint_address_to_num(device.endpoints[interface_number as usize][0 as usize].endpoint_address));

    loop {
        let result = UsbInterruptMessage(device, 1, 
            UsbPipeAddress {
                transfer_type: UsbTransfer::Interrupt,
                speed: device.speed,
                end_point: endpoint_address_to_num(device.endpoints[interface_number as usize][0 as usize].endpoint_address),
                // end_point: device.endpoints[interface_number as usize][0 as usize].endpoint_address.Number,
                device: device.number as u8,
                direction: UsbDirection::In,
                max_size: size_from_number(device.descriptor.max_packet_size0 as u32),
                _reserved: 0,
            }, 
            buffer.as_mut_ptr(), 8,
            HidMessageTimeout
        );

        let grxstsr = read_volatile(DOTG_GRXSTSRH);
        println!("| HID: GRXSTSR: {:x}", grxstsr);

        let grxstspd = read_volatile(DOTG_GRXSTSPD);
        println!("| HID: GRXSTSPD: {:x}", grxstspd);

        println!("| HID: endpoint result: {:?}", result);

        memory_copy(buffer.as_mut_ptr(), get_dwc_ptr(DOTG_DFIFO(1) as usize) as *const u8, 8);

        for i in 0..8 {
            print!("{:x} ", buffer[i]);
        }
        println!();
    
        micro_delay(500000);
    }


    

    return ResultCode::OK;
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum HidReportType {
    Input = 1,
    Output = 2,
    Feature = 3,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum UsbDeviceHidRequest {
    GetReport = 1,
    GetIdle = 2,
    GetProtocol = 3,
    SetReport = 9,
    SetIdle = 10,
    SetProtocol = 11,
}

impl UsbDeviceHidRequest {
    pub fn convert_request(request: UsbDeviceHidRequest) -> UsbDeviceRequestRequest {
        match request {
            UsbDeviceHidRequest::GetReport => UsbDeviceRequestRequest::ClearFeature,
            UsbDeviceHidRequest::GetIdle => UsbDeviceRequestRequest::GetIdle,
            UsbDeviceHidRequest::GetProtocol => UsbDeviceRequestRequest::SetFeature,
            UsbDeviceHidRequest::SetReport => UsbDeviceRequestRequest::SetConfiguration,
            UsbDeviceHidRequest::SetIdle => UsbDeviceRequestRequest::GetInterface,
            UsbDeviceHidRequest::SetProtocol => UsbDeviceRequestRequest::SetInterface,
        }
    }
}