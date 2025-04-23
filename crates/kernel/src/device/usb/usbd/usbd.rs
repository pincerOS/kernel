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
use crate::shutdown;

use alloc::boxed::Box;
use alloc::vec;

use super::super::configuration::*;
use super::super::types::*;
use super::descriptors::*;
use super::device::*;
use super::pipe::*;
use super::request::*;
use crate::device::usb::usbd::endpoint::*;
use crate::device::usb::usbd::transfer::*;

use core::ptr;

const DEBUG_DISABLE_MAX_PACKET_SIZE: bool = false;

//TODO: This needs checking under load
pub static USB_TRANSFER_QUEUE: UsbTransferQueue = UsbTransferQueue::new();

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
    ConfigurationLoad(bus);

    if size_of::<UsbDeviceRequest>() != 0x8 {
        println!("Error: UsbDeviceRequest size is not 8 bytes");
        return ResultCode::ErrorCompiler;
    }

    println!("| HcdInitialize");
    // if HcdInitialize(bus, base_addr) != ResultCode::OK {
    //     println!("Error: HcdInitialize failed");
    //     return ResultCode::ErrorDevice;
    // }

    // println!("| HcdStart");
    // if HcdStart(bus) != ResultCode::OK {
    //     println!("Error: HcdStart failed");
    //     return ResultCode::ErrorDevice;
    // }
    
    if DwcInit(bus, base_addr) != ResultCode::OK {
        println!("Error: DwcInit failed");
        return ResultCode::ErrorDevice;
    }

    let result = UsbAttachRootHub(bus);
    if result != ResultCode::OK {
        println!("Error: UsbAttachRootHub failed");
        return result;
    }

    result
}

pub unsafe fn UsbSendBulkMessage(
    device: &mut UsbDevice,
    pipe: UsbPipeAddress,
    buffer: Box<[u8]>,
    buffer_length: u32,
    packet_id: PacketId,
    device_endpoint_number: u8,
    timeout_: u32,
) -> ResultCode {
    let callback_fn = if pipe.direction == UsbDirection::In {
        finish_bulk_endpoint_callback_in
    } else {
        finish_bulk_endpoint_callback_out
    };

    let b = Box::new(UsbXfer {
        endpoint_descriptor: endpoint_descriptor {
            endpoint_address: pipe.end_point,
            endpoint_direction: pipe.direction,
            endpoint_type: pipe.transfer_type,
            max_packet_size: pipe.max_size,
            device_endpoint_number: device_endpoint_number,
            device: device,
            device_number: device.number,
            device_speed: device.speed,
            buffer_length: buffer_length,
            // buffer: buffer,
            timeout: timeout_,
        },
        buffer: Some(buffer),
        buffer_length: buffer_length,
        callback: Some(callback_fn),
        packet_id: packet_id,
        pipe: pipe,
    });

    let result;
    result = USB_TRANSFER_QUEUE.add_transfer(b, pipe.transfer_type);
    if result.is_err() {
        panic!("| USBD: Failed to add transfer to queue");
    }

    let available_channel = dwc_otg_get_active_channel();
    if available_channel == ChannelCount as u8 {
        //no available channel at the moment
        return ResultCode::OK;
    }

    unsafe {
        let transfer = USB_TRANSFER_QUEUE.get_transfer();

        if transfer.is_none() {
            dwc_otg_free_channel(available_channel as u32);
            return ResultCode::OK;
        }

        return UsbBulkMessage(device, transfer.unwrap(), available_channel);
    }
}

pub unsafe fn UsbBulkMessage(
    device: &mut UsbDevice,
    usb_xfer: Box<UsbXfer>,
    channel: u8,
) -> ResultCode {
    unsafe {
        DWC_CHANNEL_CALLBACK.endpoint_descriptors[channel as usize] =
            Some(usb_xfer.endpoint_descriptor);
        DWC_CHANNEL_CALLBACK.callback[channel as usize] = usb_xfer.callback;
    }

    let result = unsafe {
        HcdSubmitBulkMessage(
            device,
            channel,
            usb_xfer.pipe,
            usb_xfer.buffer,
            usb_xfer.buffer_length,
            usb_xfer.packet_id,
        )
    };

    if result != ResultCode::OK {
        println!("| USBD: Failed to send bulk message: {:?}", result);
        return result;
    }

    return result;
}

pub unsafe fn UsbSendInterruptMessage(
    device: &mut UsbDevice,
    pipe: UsbPipeAddress,
    buffer_length: u32,
    packet_id: PacketId,
    _timeout_: u32,
    callback: fn(endpoint_descriptor, u32, u8) -> bool,
    endpoint: endpoint_descriptor,
) -> ResultCode {
    let b = Box::new(UsbXfer {
        endpoint_descriptor: endpoint,
        buffer: None,
        buffer_length: buffer_length,
        callback: Some(callback),
        packet_id: packet_id,
        pipe: pipe,
    });

    let result;
    result = USB_TRANSFER_QUEUE.add_transfer(b, pipe.transfer_type);
    if result.is_err() {
        panic!("| USBD: Failed to add transfer to queue");
    }

    let available_channel = dwc_otg_get_active_channel();
    if available_channel == ChannelCount as u8 {
        //no available channel at the moment
        return ResultCode::OK;
    }

    unsafe {
        let transfer = USB_TRANSFER_QUEUE.get_transfer();

        if transfer.is_none() {
            dwc_otg_free_channel(available_channel as u32);
            return ResultCode::OK;
        }

        return UsbInterruptMessage(device, transfer.unwrap(), available_channel);
    }
}

pub unsafe fn UsbInterruptMessage(
    device: &mut UsbDevice,
    usb_xfer: Box<UsbXfer>,
    channel: u8,
) -> ResultCode {
    unsafe {
        DWC_CHANNEL_CALLBACK.callback[channel as usize] = Some(usb_xfer.callback.unwrap());
        DWC_CHANNEL_CALLBACK.endpoint_descriptors[channel as usize] =
            Some(usb_xfer.endpoint_descriptor);
    }

    let result = unsafe {
        HcdSubmitInterruptMessage(
            device,
            channel,
            usb_xfer.pipe,
            usb_xfer.buffer_length,
            usb_xfer.packet_id,
        )
    };

    if result != ResultCode::OK {
        println!("| USBD: Failed to send interrupt message: {:?}", result);
        return result;
    }

    return result;
}

pub unsafe fn UsbControlMessage(
    device: &mut UsbDevice,
    pipe: UsbPipeAddress,
    buffer: *mut u8,
    buffer_length: u32,
    request: &mut UsbDeviceRequest,
    timeout_: u32,
) -> ResultCode {
    let result = unsafe { HcdSubmitControlMessage(device, pipe, buffer, buffer_length, request) };

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

pub unsafe fn UsbGetDescriptor(
    device: &mut UsbDevice,
    desc_type: DescriptorType,
    index: u8,
    langId: u16,
    buffer: *mut u8,
    length: u32,
    minimumLength: u32,
    recipient: u8,
) -> ResultCode {
    let result;
    println!("| USBD: Getting descriptor at device {}", device.number);
    result = unsafe {
        UsbControlMessage(
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
            // minimumLength,
            &mut UsbDeviceRequest {
                request_type: 0x80 | recipient,
                request: UsbDeviceRequestRequest::GetDescriptor,
                value: (desc_type as u16) << 8 | index as u16,
                index: langId,
                length: length as u16,
            },
            ControlMessageTimeout as u32,
        )
    };

    if result != ResultCode::OK {
        println!("| USBD: Failed to get descriptor {} {}", device.last_transfer, minimumLength);
        return result;
    }

    if device.last_transfer < minimumLength {
        println!("| USBD: Descriptor too short {} {}", device.last_transfer, minimumLength);
        printDWCErrors(0);
        return ResultCode::ErrorDevice;
    }

    return ResultCode::OK;
}

fn UsbReadDeviceDescriptor(device: &mut UsbDevice) -> ResultCode {
    let result;
    let descriptor_ptr = &mut device.descriptor as *mut UsbDeviceDescriptor as *mut u8;
    println!("| UsbReadDeviceDescriptor; speed: {:?}", device.speed);
    if device.speed == UsbSpeed::Low {
        if !DEBUG_DISABLE_MAX_PACKET_SIZE {
            device.descriptor.max_packet_size0 = 8;
        }
        result = unsafe {
            UsbGetDescriptor(
                device,
                DescriptorType::Device,
                0,
                0,
                descriptor_ptr,
                size_of::<UsbDeviceDescriptor>() as u32,
                8,
                0,
            )
        };
        if result != ResultCode::OK {
            return result;
        }

        if device.last_transfer == size_of::<UsbDeviceDescriptor>() as u32 {
            return result;
        }
        return unsafe {
            UsbGetDescriptor(
                device,
                DescriptorType::Device,
                0,
                0,
                descriptor_ptr,
                size_of::<UsbDeviceDescriptor>() as u32,
                size_of::<UsbDeviceDescriptor>() as u32,
                0,
            )
        };
    } else if device.speed == UsbSpeed::Full {
        if !DEBUG_DISABLE_MAX_PACKET_SIZE {
            device.descriptor.max_packet_size0 = 64;
            // device.descriptor.max_packet_size0 = 8;
        }
        result = unsafe {
            UsbGetDescriptor(
                device,
                DescriptorType::Device,
                0,
                0,
                descriptor_ptr,
                size_of::<UsbDeviceDescriptor>() as u32,
                8,
                0,
            )
        };
        if result != ResultCode::OK {

            //print the device descriptor
            println!("| USBD: failed device descriptor {:?}", device.descriptor);

            return result;
        }
        println!("//RIGHT HERE!");

        if device.last_transfer == size_of::<UsbDeviceDescriptor>() as u32 {
            return result;
        }
        return unsafe {
            UsbGetDescriptor(
                device,
                DescriptorType::Device,
                0,
                0,
                descriptor_ptr,
                size_of::<UsbDeviceDescriptor>() as u32,
                size_of::<UsbDeviceDescriptor>() as u32,
                0,
            )
        };
    } else {
        if !DEBUG_DISABLE_MAX_PACKET_SIZE {
            device.descriptor.max_packet_size0 = 64;
            // device.descriptor.max_packet_size0 = 8;
        }
        return unsafe {
            UsbGetDescriptor(
                device,
                DescriptorType::Device,
                0,
                0,
                descriptor_ptr,
                size_of::<UsbDeviceDescriptor>() as u32,
                size_of::<UsbDeviceDescriptor>() as u32,
                0,
            )
        };
    }
}

fn UsbSetAddress(device: &mut UsbDevice, address: u8) -> ResultCode {
    println!("| USBD: Set device address to {}", address);
    if device.status != UsbDeviceStatus::Default {
        println!("| USBD: Device not in default state");
        return ResultCode::ErrorDevice;
    }

    let result = unsafe {
        UsbControlMessage(
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
        )
    };

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

    let result = unsafe {
        UsbControlMessage(
            device,
            UsbPipeAddress {
                transfer_type: UsbTransfer::Control,
                speed: device.speed,
                end_point: 0,
                direction: UsbDirection::Out,
                device: device.number as u8,
                max_size: size_from_number(device.descriptor.max_packet_size0 as u32),
                _reserved: 0,
            },
            ptr::null_mut(),
            0,
            &mut UsbDeviceRequest {
                request_type: 0,
                request: UsbDeviceRequestRequest::SetConfiguration,
                value: configuration as u16,
                index: 0,
                length: 0,
            },
            ControlMessageTimeout as u32,
        )
    };

    if result != ResultCode::OK {
        println!("| USBD: Failed to set configuration");
        return result;
    }

    device.configuration_index = configuration;
    device.status = UsbDeviceStatus::Configured;

    return ResultCode::OK;
}

fn UsbConfigure(device: &mut UsbDevice, configuration: u8) -> ResultCode {
    let configuration_val;
    if device.status != UsbDeviceStatus::Addressed {
        println!("| USBD: Device not in addressed state");
        return ResultCode::ErrorDevice;
    }

    let configuration_ptr = &mut device.configuration as *mut UsbConfigurationDescriptor as *mut u8;
    let mut result = unsafe {
        UsbGetDescriptor(
            device,
            DescriptorType::Configuration,
            configuration,
            0,
            configuration_ptr,
            size_of::<UsbConfigurationDescriptor>() as u32,
            size_of::<UsbConfigurationDescriptor>() as u32,
            0,
        )
    };
    if result != ResultCode::OK {
        println!("| USBD: Failed to get configuration descriptor");
        return result;
    }

    // let configuration_dev = &mut device.configuration;
    // println!(
    //     "| USBD: Configuration descriptor:\n {:#?}",
    //     configuration_dev
    // );

    //TODO TODO: if ((fullDescriptor = MemoryAllocate(device->Configuration.TotalLength)) == NULL) {
    // LOG("USBD: Failed to allocate space for descriptor.\n");
    // return ErrorMemory;
    let config_total_length = device.configuration.total_length;
    // println!(
    //     "| USBD: Configuration descriptor length: {}",
    //     config_total_length
    // );
    let mut fullDescriptor_vec = vec![0; config_total_length as usize].into_boxed_slice();
    let fullDescriptor = fullDescriptor_vec.as_mut_ptr() as *mut u8;

    result = unsafe {
        UsbGetDescriptor(
            device,
            DescriptorType::Configuration,
            configuration,
            0,
            fullDescriptor,
            device.configuration.total_length as u32,
            device.configuration.total_length as u32,
            0,
        )
    };
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
                        memory_copy(
                            &mut device.interfaces[last_interface] as *mut UsbInterfaceDescriptor
                                as *mut u8,
                            interface as *const u8,
                            size_of::<UsbInterfaceDescriptor>(),
                        );
                        last_endpoint = 0;
                        is_alternate = false;
                    } else {
                        is_alternate = true;
                    }
                }
                DescriptorType::Endpoint => {
                    if is_alternate {
                        continue;
                    }
                    if last_interface == MAX_INTERFACES_PER_DEVICE
                        || last_endpoint
                            >= device.interfaces[last_interface].endpoint_count as usize
                    {
                        println!("| USBD: Unexpected endpoint descriptor interface");
                        return ResultCode::ErrorDevice;
                    }
                    let endpoint = header as *mut UsbEndpointDescriptor;
                    memory_copy(
                        &mut device.endpoints[last_interface][last_endpoint]
                            as *mut UsbEndpointDescriptor as *mut u8,
                        endpoint as *const u8,
                        size_of::<UsbEndpointDescriptor>(),
                    );
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
    let bus = unsafe { &mut *(device.bus) };

    println!("| USBD: Attaching device {}", device.number);

    let address = device.number;
    device.number = 0;

    let mut result = UsbReadDeviceDescriptor(device);
    //print USB device descriptor
    println!("| USBD: Device descriptor: {:?}", device.descriptor);
    if result != ResultCode::OK {
        println!("| USBD: Failed to read device descriptor");

        for it in 0..100 {
            micro_delay(400000);
            result = UsbReadDeviceDescriptor(device);
            if result == ResultCode::OK {
                println!("| SUCCESS: Read device descriptor at iteration {}", it);
                break;
            }
            println!("| USBD: Failed to read device descriptor at iteration {}", it);
        }

        if result != ResultCode::OK {
            return result;
        }
    }


    device.status = UsbDeviceStatus::Default;

    // if let Some(parent) = device.parent {
    //     unsafe {
    //         if let Some(device_child_reset) = (*parent).device_child_reset {
    //             result = device_child_reset(&mut *parent, device);
    //             if result != ResultCode::OK {
    //                 println!("| USBD: Failed to reset parent device");
    //                 return result;
    //             }
    //         }
    //     }
    // } else {
    //     println!("| USBD: No parent device");
    // }

    result = UsbSetAddress(device, address as u8);
    if result != ResultCode::OK {
        println!("| USBD: Failed to set address");
        device.number = address;
        return result;
    }

    device.number = address;
    result = UsbReadDeviceDescriptor(device);

    for it in 0..100 {
        if result == ResultCode::OK {
            break;
        }
        println!("| USBD: Failed to read device descriptor at iteration {}", it);
        micro_delay(100000);
    }

    if result != ResultCode::OK {
        println!("| USBD: Failed to read device descriptor");
        return result;
    }
    println!("| USBD: Device descriptor: {:?}", device.descriptor);

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

    println!(
        "\n Device interface class: {} at device number {}\n",
        device.interfaces[0].class as u16, device.number
    );

    if (device.interfaces[0].class as usize) < INTERFACE_CLASS_ATTACH_COUNT {
        // for j in 0..device.configuration.interface_count {
        //     println!(
        //         "| USBD: Device interface {}:\n {:?}",
        //         j, device.interfaces[j as usize]
        //     );
        //     for i in 0..device.interfaces[j as usize].endpoint_count {
        //         println!(
        //             "| USBD: Endpoint descriptor {} {}:\n {:#?}",
        //             j, i, device.endpoints[j as usize][i as usize]
        //         );
        //     }
        // }
        if let Some(class_attach) = bus.interface_class_attach[device.interfaces[0].class as usize]
        {
            result = class_attach(device, 0);
            if result != ResultCode::OK {
                println!("| USBD: Class attach handler failed");
                return result;
            }
        } else {
            println!("| USBD: No class attach handler");
            shutdown();
        }
    } else {
        println!("| USBD: Invalid interface class");
    }

    return ResultCode::OK;
}

pub fn UsbAllocateDevice(mut devices: Box<UsbDevice>) -> ResultCode {
    let bus = unsafe { &mut *(devices.bus) };
    let device = devices.as_mut();
    device.status = UsbDeviceStatus::Attached;
    device.error = UsbTransferError::NoError;
    device.port_number = 0;
    device.configuration_index = 0xff;
    device.descriptor.max_packet_size0 = 8;

    for number in 0..MaximumDevices {
        if bus.devices[number].is_none() {
            // println!("| USBD: Allocating device {}", number);
            device.number = number as u32 + 1;
            if device.number == 1 {
                //Roothub -> get speed
                use crate::device::usb::hcd::dwc::dwc_otgreg::DOTG_HPRT;
                let hprt = read_volatile(DOTG_HPRT);
                let speed = match (hprt >> 17) & 0b11 {
                    0b00 => {
                        device.speed = UsbSpeed::High;
                        // device.speed = UsbSpeed::Full;
                        "High-Speed"
                    },
                    0b01 => {
                        device.speed = UsbSpeed::Full;
                        "Full-Speed"
                    },
                    0b10 => {
                        device.speed = UsbSpeed::Low;
                        "Low-Speed"
                    },
                    _ => { panic!("| USBD: Unknown speed") },
                };

                println!("| USBD: Roothub: {}", speed);
            }
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

    let device = unsafe { Box::new(UsbDevice::new(bus, 0 as u32)) };

    if UsbAllocateDevice(device) != ResultCode::OK {
        println!("Error: UsbAllocateDevice failed");
        return ResultCode::ErrorMemory;
    }

    bus.devices[0].as_mut().unwrap().status = UsbDeviceStatus::Powered;
    bus.devices[0].as_mut().unwrap().speed = UsbSpeed::High;

    return UsbAttachDevice(&mut (bus.devices[0].as_mut().unwrap()));
}

// pub fn UsbCheckForChange(bus: &mut UsbBus) {
//     if bus.devices[RootHubDeviceNumber].is_none() {
//         return;
//     }

//     if let Some(device_check_for_change) = bus.devices[RootHubDeviceNumber].as_mut().unwrap().device_check_for_change {
//         device_check_for_change(bus.devices[RootHubDeviceNumber].as_mut().unwrap().as_mut());
//     }
// }
