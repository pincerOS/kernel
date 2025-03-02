/******************************************************************************
*	usbd/usbd.c
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*	usbd.c contains code relating to the generic USB driver. USB
*	is designed such that this driver's interface would be virtually the same
*	across all systems, and in fact its implementation varies little either.
******************************************************************************/

use crate::device::system_timer::micro_delay;
use crate::device::usb::hcd::dwc::dwc_otg::*;
use crate::device::usb::hcd::dwc::roothub::memory_copy;

use alloc::boxed::Box;
use alloc::vec;

use super::super::configuration::*;
use super::super::types::*;
use super::descriptors::*;
use super::device::*;
use super::pipe::*;
use super::request::*;

use core::ptr;
use core::time;

/** The default timeout in ms of control transfers. */
pub const ControlMessageTimeout: usize = 10;

pub fn UsbLoad(bus: &mut UsbBus) {
    for i in 0..MaximumDevices {
        bus.devices[i] = None
    }

    for i in 0..INTERFACE_CLASS_ATTACH_COUNT {
        bus.interface_class_attach[i] = None
    }
}

pub fn UsbInitialise(bus: &mut UsbBus, base_addr: *mut ()) -> ResultCode {
    let mut result = ResultCode::OK;

    ConfigurationLoad(bus);

    if size_of::<UsbDeviceRequest>() != 0x8 {
        println!("Error: UsbDeviceRequest size is not 8 bytes");
        return ResultCode::ErrorCompiler;
    }

    if HcdInitialize(bus, base_addr) != ResultCode::OK {
        println!("Error: HcdInitialize failed");
        return ResultCode::ErrorDevice;
    }

    if HcdStart(bus) != ResultCode::OK {
        println!("Error: HcdStart failed");
        return ResultCode::ErrorDevice;
    }

    result = UsbAttachRootHub(bus);
    if result != ResultCode::OK {
        println!("Error: UsbAttachRootHub failed");
        return result;
    }

    result
}

pub fn UsbControlMessage(
    device: &mut UsbDevice,
    pipe: UsbPipeAddress,
    buffer: *mut u8,
    buffer_length: u32,
    request: &mut UsbDeviceRequest,
    timeout_: u32,
) -> ResultCode {
    let mut result = HcdSubmitControlMessage(device, pipe, buffer, buffer_length, request);

    if result != ResultCode::OK {
        println!("| Failed to send message");
        return result;
    }
    let mut timeout = timeout_;
    while timeout > 0 && (device.error == UsbTransferError::Processing) {
        timeout -= 1;
    }

    if device.error == UsbTransferError::Processing {
        println!("| USBD Message Timeout");
        return ResultCode::ErrorTimeout;
    }

    if device.error != UsbTransferError::NoError {
        println!("| USBD Message Error. TBD");
        //TODO: GO DO
        return ResultCode::ErrorDevice;
    }

    return result;
}

pub fn UsbGetDescriptor(
    device: &mut UsbDevice,
    desc_type: DescriptorType,
    index: u8,
    langId: u16,
    buffer: *mut u8,
    length: u32,
    minimumLength: u32,
    recipient: u8,
) -> ResultCode {
    let mut result;
    println!("| USBD: Getting descriptor at device {}", device.number);
    result = UsbControlMessage(
        device,
        UsbPipeAddress {
            max_size: size_from_number(device.descriptor.max_packet_size0 as u32),
            speed: device.speed,
            end_point: 0,
            device: device.number as u8,
            transfer_type: UsbTransfer::Control,
            direction: UsbDirection::In,
            _reserved: 0,
        },
        buffer,
        length,
        &mut UsbDeviceRequest {
            request_type: 0x80 | recipient,
            request: UsbDeviceRequestRequest::GetDescriptor,
            value: (desc_type as u16) << 8 | index as u16,
            index: langId,
            length: length as u16,
        },
        ControlMessageTimeout as u32,
    );

    if result != ResultCode::OK {
        println!("| USBD: Failed to get descriptor");
        return result;
    }

    if device.last_transfer < minimumLength {
        println!("| USBD: Descriptor too short");
        return ResultCode::ErrorDevice;
    }

    return ResultCode::OK;
}

fn UsbReadDeviceDescriptor(device: &mut UsbDevice) -> ResultCode {
    let result;
    let descriptor_ptr = &mut device.descriptor as *mut UsbDeviceDescriptor as *mut u8;
    if device.speed == UsbSpeed::Low {
        device.descriptor.max_packet_size0 = 8;
        result = UsbGetDescriptor(
            device,
            DescriptorType::Device,
            0,
            0,
            descriptor_ptr,
            size_of::<UsbDeviceDescriptor>() as u32,
            8,
            0,
        );
        if result != ResultCode::OK {
            return result;
        }

        if device.last_transfer == size_of::<UsbDeviceDescriptor>() as u32 {
            return result;
        }
        return UsbGetDescriptor(
            device,
            DescriptorType::Device,
            0,
            0,
            descriptor_ptr,
            size_of::<UsbDeviceDescriptor>() as u32,
            size_of::<UsbDeviceDescriptor>() as u32,
            0,
        );
    } else if device.speed == UsbSpeed::Full {
        // device.descriptor.max_packet_size0 = 64;
        device.descriptor.max_packet_size0 = 8;
        result = UsbGetDescriptor(
            device,
            DescriptorType::Device,
            0,
            0,
            descriptor_ptr,
            size_of::<UsbDeviceDescriptor>() as u32,
            8,
            0,
        );
        if result != ResultCode::OK {
            return result;
        }

        if device.last_transfer == size_of::<UsbDeviceDescriptor>() as u32 {
            return result;
        }
        return UsbGetDescriptor(
            device,
            DescriptorType::Device,
            0,
            0,
            descriptor_ptr,
            size_of::<UsbDeviceDescriptor>() as u32,
            size_of::<UsbDeviceDescriptor>() as u32,
            0,
        );
    } else {
        // device.descriptor.max_packet_size0 = 64;
        device.descriptor.max_packet_size0 = 8;
        return UsbGetDescriptor(
            device,
            DescriptorType::Device,
            0,
            0,
            descriptor_ptr,
            size_of::<UsbDeviceDescriptor>() as u32,
            size_of::<UsbDeviceDescriptor>() as u32,
            0,
        );
    }
}

fn UsbSetAddress(device: &mut UsbDevice, address: u8) -> ResultCode {
    println!("| USBD: Set device address to {}", address);
    if device.status != UsbDeviceStatus::Default {
        println!("| USBD: Device not in default state");
        return ResultCode::ErrorDevice;
    }

    let result = UsbControlMessage(
        device,
        UsbPipeAddress {
            max_size: size_from_number(device.descriptor.max_packet_size0 as u32),
            speed: device.speed,
            end_point: 0,
            device: 0,
            transfer_type: UsbTransfer::Control,
            direction: UsbDirection::Out,
            _reserved: 0,
        },
        ptr::null_mut(),
        0,
        &mut UsbDeviceRequest {
            request_type: 0,
            request: UsbDeviceRequestRequest::SetAddress,
            value: address as u16,
            index: 0,
            length: 0,
        },
        ControlMessageTimeout as u32,
    );

    if result != ResultCode::OK {
        println!("| USBD: Failed to set address");
        return result;
    }
    micro_delay(10000);

    device.number = address as u32;
    device.status = UsbDeviceStatus::Addressed;

    return ResultCode::OK;
}

fn UsbSetConfigure(device: &mut UsbDevice, configuration: u8) -> ResultCode {
    if device.status != UsbDeviceStatus::Addressed {
        println!("| USBD: Device not in addressed state");
        return ResultCode::ErrorDevice;
    }

    let mut result = UsbControlMessage(device, 
        UsbPipeAddress {
            transfer_type: UsbTransfer::Control,
            speed: device.speed,
            end_point: 0,
            direction: UsbDirection::Out,
            device: device.number as u8,
            max_size: size_from_number(device.descriptor.max_packet_size0 as u32),
            _reserved: 0,
        }, ptr::null_mut(), 0, 
        &mut UsbDeviceRequest {
            request_type: 0,
            request: UsbDeviceRequestRequest::SetConfiguration,
            value: configuration as u16,
            index: 0,
            length: 0,
        }, ControlMessageTimeout as u32);
    if result != ResultCode::OK {
        println!("| USBD: Failed to set configuration");
        return result;
    }

    device.configuration_index = configuration;
    device.status = UsbDeviceStatus::Configured;

    return ResultCode::OK;
}

fn UsbConfigure(device: &mut UsbDevice, configuration: u8) -> ResultCode {

    let mut configuration_val = configuration;
    if device.status != UsbDeviceStatus::Addressed {
        println!("| USBD: Device not in addressed state");
        return ResultCode::ErrorDevice;
    }

    let configuration_ptr = &mut device.configuration as *mut UsbConfigurationDescriptor as *mut u8;
    let mut result = UsbGetDescriptor(device, DescriptorType::Configuration, configuration, 0, configuration_ptr, size_of::<UsbConfigurationDescriptor>() as u32, size_of::<UsbConfigurationDescriptor>() as u32, 0);
    if result != ResultCode::OK {
        println!("| USBD: Failed to get configuration descriptor");
        return result;
    }

    let configuration_dev = &mut device.configuration;
    println!("| USBD: Configuration descriptor:\n {:#?}", configuration_dev);

    //TODO TODO: if ((fullDescriptor = MemoryAllocate(device->Configuration.TotalLength)) == NULL) {
		// LOG("USBD: Failed to allocate space for descriptor.\n");
		// return ErrorMemory;
    let config_total_length = device.configuration.total_length;
    println!("| USBD: Configuration descriptor length: {}", config_total_length);
    let mut fullDescriptor_vec = vec![0; config_total_length as usize].into_boxed_slice();
    let mut fullDescriptor = fullDescriptor_vec.as_mut_ptr() as *mut u8;

    result = UsbGetDescriptor(device, DescriptorType::Configuration, configuration, 0, fullDescriptor, device.configuration.total_length as u32, device.configuration.total_length as u32, 0);
    if result != ResultCode::OK {
        println!("| USBD: Failed to get full configuration descriptor");
        return result;
    }

    device.configuration_index = configuration;
    configuration_val = device.configuration.configuration_value;

    let mut header = fullDescriptor as *mut UsbDescriptorHeader;
    let mut last_interface = MAX_INTERFACES_PER_DEVICE;
    let mut last_endpoint = MAX_ENDPOINTS_PER_DEVICE;
    let mut is_alternate = false;
    let end = (fullDescriptor as usize) + device.configuration.total_length as usize;
    header = unsafe { header.byte_add((*header).descriptor_length as usize) };
    while (header as usize) < end {
        unsafe {
            match (*header).descriptor_type {
                DescriptorType::Interface => {
                    let interface = header as *mut UsbInterfaceDescriptor;
                    if last_interface != (*interface).number as usize {
                        last_interface = (*interface).number as usize;
                        memory_copy(&mut device.interfaces[last_interface] as *mut UsbInterfaceDescriptor as *mut u8, interface as *const u8, size_of::<UsbInterfaceDescriptor>());
                        last_endpoint = 0;
                        is_alternate = false;
                    } else {
                        is_alternate = true;
                    }
                }
                DescriptorType::Endpoint => {
                    if is_alternate { continue; }
                    if last_interface == MAX_INTERFACES_PER_DEVICE || last_endpoint >= device.interfaces[last_interface].endpoint_count as usize {
                        println!("| USBD: Unexpected endpoint descriptor interface");
                        return ResultCode::ErrorDevice;
                    }
                    let endpoint = header as *mut UsbEndpointDescriptor;
                    memory_copy(&mut device.endpoints[last_interface][last_endpoint] as *mut UsbEndpointDescriptor as *mut u8, endpoint as *const u8, size_of::<UsbEndpointDescriptor>());
                    last_endpoint += 1;
                }
                _ => {
                    if (*header).descriptor_length == 0 {
                        break;
                    }
                }
            }
            header = header.byte_add((*header).descriptor_length as usize);
        }
    }

    result = UsbSetConfigure(device, configuration_val);

    if result != ResultCode::OK {
        println!("| USBD: Failed to set configuration");
        return result;
    }

    device.full_configuration = Some(fullDescriptor_vec);

    return ResultCode::OK;
}

pub fn UsbAttachDevice(device: &mut UsbDevice) -> ResultCode {
    let mut bus = unsafe { &mut *(device.bus) };

    println!("| USBD: Attaching device {}", device.number);

    let address = device.number;
    device.number = 0;

    let mut result = UsbReadDeviceDescriptor(device);
    //print USB device descriptor
    println!("| USBD: Device descriptor:\n {:#?}", device.descriptor);
    if result != ResultCode::OK {
        println!("| USBD: Failed to read device descriptor");
        return result;
    }
    device.status = UsbDeviceStatus::Default;

    if let Some(parent) = device.parent {
        unsafe {
            if let Some(device_child_reset) = (*parent).device_child_reset {
                result = device_child_reset(&mut *parent, device);
                if result != ResultCode::OK {
                    println!("| USBD: Failed to reset parent device");
                    return result;
                }
            }
        }
    } else {
        println!("| USBD: No parent device");
    }

    result = UsbSetAddress(device, address as u8);
    if result != ResultCode::OK {
        println!("| USBD: Failed to set address");
        device.number = address;
        return result;
    }

    device.number = address;
    result = UsbReadDeviceDescriptor(device);
    if result != ResultCode::OK {
        println!("| USBD: Failed to read device descriptor");
        return result;
    }
    println!("| USBD: Device descriptor:\n {:#?}", device.descriptor);

    let vendor_id = device.descriptor.vendor_id;
    let product_id = device.descriptor.product_id;
    println!(
        "| USBD: Device attached, vid: {:#06x}, pid: {:#06x}",
        vendor_id, product_id
    );

    
    result = UsbConfigure(device, 0);
    if result != ResultCode::OK {
        println!("| USBD: Failed to configure device");
        return result;
    }

    println!("\n Device interface class: {} at device number {}\n", device.interfaces[0].class as u16, device.number);
    

    if (device.interfaces[0].class as usize) < INTERFACE_CLASS_ATTACH_COUNT {
        if let Some(class_attach) = bus.interface_class_attach[device.interfaces[0].class as usize] {
            result = class_attach(device, 0);
            // micro_delay(1000000);
            if result != ResultCode::OK {
                println!("| USBD: Class attach handler failed");
                return result;
            }
        } else {
            println!("| USBD: No class attach handler");
        }
    } else {
        println!("| USBD: Invalid interface class");
    }

    return ResultCode::OK;
}

pub fn UsbAllocateDevice(devices: &mut Box<UsbDevice>) -> ResultCode {
    let mut bus = unsafe { &mut *(devices.bus) };
    let mut device = devices.as_mut();
    device.status = UsbDeviceStatus::Attached;
    device.error = UsbTransferError::NoError;
    device.port_number = 0;
    device.configuration_index = 0xff;

    for number in 0..MaximumDevices {
        if bus.devices[number].is_none() {
            println!("| USBD: Allocating device {}", number);
            device.number = number as u32 + 1;
            bus.devices[number] = Some(devices);
            break;
        }
    }

    return ResultCode::OK;
}

fn UsbAttachRootHub(bus: &mut UsbBus) -> ResultCode {
    println!("| USBD: Attaching root hub");
    if bus.devices[0].is_some() {
        println!("Error: Root hub already attached");
        return ResultCode::ErrorDevice;
    }

    let mut device = Box::new(UsbDevice::new(bus, 0 as u32));

    if UsbAllocateDevice(&mut device) != ResultCode::OK {
        println!("Error: UsbAllocateDevice failed");
        return ResultCode::ErrorMemory;
    }

    unsafe { (*bus.devices[0].unwrap()).status = UsbDeviceStatus::Powered };

    return UsbAttachDevice(unsafe {&mut (*bus.devices[0].unwrap()) });
}


// pub fn UsbCheckForChange(bus: &mut UsbBus) {
//     if bus.devices[RootHubDeviceNumber].is_none() {
//         return;
//     }

//     if let Some(device_check_for_change) = bus.devices[RootHubDeviceNumber].as_mut().unwrap().device_check_for_change {
//         device_check_for_change(bus.devices[RootHubDeviceNumber].as_mut().unwrap().as_mut());
//     }
// }