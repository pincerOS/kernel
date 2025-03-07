/******************************************************************************
*	device/hub.c
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*	device/hub.c contains code relating to the generic USB driver's hubs. USB
*	is designed such that this driver's interface would be virtually the same
*	across all systems, and in fact its implementation varies little either.
******************************************************************************/

use super::super::usbd::device::*;
use super::super::usbd::descriptors::*;
use super::super::usbd::usbd::*;

use alloc::vec;
use alloc::boxed::Box;
use crate::device::system_timer::micro_delay;
use crate::device::usb::types::*;
use crate::device::usb::hcd::hub::*;
use crate::device::usb::usbd::pipe::*;
use crate::device::usb::usbd::request::*;

pub fn HubLoad(bus: &mut UsbBus) {
    bus.interface_class_attach[InterfaceClass::InterfaceClassHub as usize] = Some(HubAttach);
}

fn HubReadDescriptor(device: &mut UsbDevice) -> ResultCode {

    let mut header = UsbDescriptorHeader::default();

    let mut result = UsbGetDescriptor(device, DescriptorType::Hub, 0, 0, &mut header as *mut UsbDescriptorHeader as *mut u8, size_of::<UsbDescriptorHeader>() as u32, size_of::<UsbDescriptorHeader>() as u32, 0x20);

    if result != ResultCode::OK {
        println!("| HUB: failed to read descriptor");
        return result;
    }

    let mut hub = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut HubDevice) };
    if hub.Descriptor.is_none() {
        println!("| HUB: allocating descriptor of size {} with HubDescriptor {}", header.descriptor_length, size_of::<HubDescriptor>());
        hub.Descriptor = Some(Box::new(HubDescriptor::default()));

        //TODO: Update this creation as well
    }

    println!("Hub descriptor address {:#x}", hub.Descriptor.as_mut().unwrap().as_mut() as *mut HubDescriptor as usize);
    result = UsbGetDescriptor(device, DescriptorType::Hub, 0, 0, hub.Descriptor.as_mut().unwrap().as_mut() as *mut HubDescriptor as *mut u8, header.descriptor_length as u32, header.descriptor_length as u32, 0x20);
    if result != ResultCode::OK {
        println!("| HUB: failed to read full descriptor");
        return result;
    }

    return ResultCode::OK;
}

fn HubGetStatus(device: &mut UsbDevice) -> ResultCode {
    let mut hub = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut HubDevice) };
    let mut status = (&mut hub.Status as *mut HubFullStatus) as *mut u8;
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
        status,
        size_of::<HubFullStatus>() as u32,
        &mut UsbDeviceRequest {
            request_type: 0xa0,
            request: UsbDeviceRequestRequest::GetStatus,
            length: size_of::<HubFullStatus>() as u16,
            value: 0,
            index: 0,
        },
        ControlMessageTimeout as u32
    );

    if result != ResultCode::OK {
        println!("| HUB: failed to get hub status");
        return result;
    }

    if device.last_transfer < size_of::<HubFullStatus>() as u32 {
        println!("| HUB: failed to get hub status");
        return ResultCode::ErrorDevice;
    }

    return ResultCode::OK;
}

fn HubChangePortFeature(device: &mut UsbDevice, feature: HubPortFeature, port: u8, set: bool) -> ResultCode{
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
            request_type: 0x23,
            request: if set { UsbDeviceRequestRequest::SetFeature } else { UsbDeviceRequestRequest::ClearFeature },
            length: 0,
            value: feature as u16,
            index: (port + 1) as u16,
        },
        ControlMessageTimeout as u32
    );

    return result;
}

fn HubPowerOn(device: &mut UsbDevice) -> ResultCode {
    let data = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut HubDevice) };
    let hub_desc =  data.Descriptor.as_mut().unwrap().as_mut(); //unsafe { &mut *(.as_mut_ptr() as *mut HubDescriptor) };

    for i in 0..data.MaxChildren {
        if HubChangePortFeature(device, HubPortFeature::FeaturePower, i as u8, true) != ResultCode::OK {
            println!("| HUB: failed to power on port {}", i);
        }
    }

    micro_delay(hub_desc.PowerGoodDelay as u32);

    return ResultCode::OK;
}

fn HubPortGetStatus(device: &mut UsbDevice, port: u8) -> ResultCode {
    let mut hub = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut HubDevice) };
    let mut port_status = &mut hub.PortStatus[port as usize] as *mut HubPortFullStatus as *mut u8;
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
        port_status, 
        size_of::<HubPortFullStatus>() as u32, 
        &mut UsbDeviceRequest {
            request: UsbDeviceRequestRequest::GetStatus,
            request_type: 0xa3,
            value: 0,
            index: (port + 1) as u16,
            length: size_of::<HubPortFullStatus>() as u16,
        }, ControlMessageTimeout as u32);
    
    if result != ResultCode::OK {
        println!("| HUB: failed to get hub port status");
        return result;
    }

    if device.last_transfer < size_of::<HubPortFullStatus>() as u32 {
        println!("| HUB: failed to read hub port status");
        return ResultCode::ErrorDevice;
    }

    return ResultCode::OK;
}

fn HubPortReset(device: &mut UsbDevice, port: u8) -> ResultCode {
    let mut data = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut HubDevice) };
    let mut portStatus = &mut data.PortStatus[port as usize];

    let mut result;
    let mut retry_max= 0;
    for retry in 0..3 {
        retry_max = retry;
        result = HubChangePortFeature(device, HubPortFeature::FeatureReset, port, true);

        if result != ResultCode::OK {
            println!("| HUB: failed to reset port {}", port + 1);
            return result;
        }

        let mut timeout = 0;
        while timeout < 10 {
            timeout += 1;
            micro_delay(20000);

            result = HubPortGetStatus(device, port);
            if result != ResultCode::OK {
                println!("| HUB: failed to get status (4) for port {}", port + 1);
                return result;
            }

            let port_changed = portStatus.Change;
            let port_status = portStatus.Status;
            if port_changed.contains(HubPortStatusChange::ResetChanged) || port_status.contains(HubPortStatus::Enabled) {
                break;
            }
        }

        if timeout == 10 {
            continue;
        }

        let port_change = portStatus.Change;
        let port_status = portStatus.Status;
        if port_change.contains(HubPortStatusChange::ConnectedChanged) || !port_status.contains(HubPortStatus::Connected) {
            return ResultCode::ErrorDevice;
        }

        if port_status.contains(HubPortStatus::Enabled) {
            break;
        }

    }

    if retry_max == 3 {
        println!("| HUB: failed to reset port {}", port + 1);
        return ResultCode::ErrorDevice;
    }

    result = HubChangePortFeature(device, HubPortFeature::FeatureReset, port, false);
    if result != ResultCode::OK {
        println!("| HUB: failed to clear reset for port {}", port + 1);
    }

    return ResultCode::OK;
}

fn HubChildReset(device: &mut UsbDevice, child: &mut UsbDevice) -> ResultCode {
    let mut data = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut HubDevice) };
    println!("data {:#x} device {:#x}", data as *mut HubDevice as usize, device as *mut UsbDevice as usize);
    println!("child.parent address {:#x} device address {:#x}", child.parent.unwrap() as usize, device as *mut UsbDevice as usize);
    println!("child.port_number {} data.MaxChildren {}", child.port_number, data.MaxChildren as u8);
    println!("data.Children[child.port_number as usize] {:#x} child {:#x}", data.Children[child.port_number as usize] as usize, child as *mut UsbDevice as usize);
    data.MaxChildren = 1;
    println!("are equal {} {} {} {}", child.parent == Some(device), child.port_number >= 0, child.port_number < data.MaxChildren as u8, data.Children[child.port_number as usize] == child);
    if child.parent == Some(device) && child.port_number >= 0 && child.port_number < data.MaxChildren as u8 && data.Children[child.port_number as usize] == child {
        return HubPortReset(device, child.port_number);
    } else {
        println!("| HUB: child reset failed");
        return ResultCode::ErrorArgument;
    }
}

fn HubPortConnectionChanged(device: &mut UsbDevice, port: u8) -> ResultCode {
    let mut data = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut HubDevice) };
    println!("Address of data {:#x}", data as *mut HubDevice as usize);
    println!("Address of data.MaxChildren {:#x}", &data.MaxChildren as *const u32 as usize);
    let mut portStatus = &mut data.PortStatus[port as usize];
    println!("Max1 children: {}", data.MaxChildren);
    let mut result = HubPortGetStatus(device, port);

    if result != ResultCode::OK {
        println!("| HUB: failed to get status (2) for port {}", port + 1);
        return result;
    }

    println!("| HUB: port {} status: {:#x}", port + 1, 10);
    println!("Max3 children: {}", data.MaxChildren);

    result = HubChangePortFeature(device, HubPortFeature::FeatureConnectionChange, port, false);

    if result != ResultCode::OK {
        println!("| HUB: failed to clear connection change for port {}", port + 1);
        return result;
    }
    println!("Max4 children: {}", data.MaxChildren);
    let port_status = portStatus.Status;
    if (!(port_status.contains(HubPortStatus::Connected)) && !(port_status.contains(HubPortStatus::Enabled))) || !data.Children[port as usize].is_null() {
        println!("| HUB: Disconnected");

        println!("| HUB: PORT CONNECTION CHANGED NOT IMPLEMENTED");
        return ResultCode::ErrorIncompatible;
    }

    result = HubPortReset(device, port);
    if result != ResultCode::OK {
        println!("| HUB: count not reset port {} for new device", port + 1);
        return result;
    }
    println!("Max5 children: {}", data.MaxChildren);
    
    
    let mut dev = Box::new(UsbDevice::new(device.bus, 0));
    println!("Max5.2 children: {}, Box address {:#x}", data.MaxChildren, dev.as_mut() as *mut UsbDevice as usize);
    data.Children[port as usize] = dev.as_mut() as *mut UsbDevice;
    println!("data children address {:#x} dev address {:#x}", data.Children[port as usize] as usize, dev.as_mut() as *mut UsbDevice as usize);
    result = UsbAllocateDevice(&mut dev);
    println!("Max5.5 children: {}", data.MaxChildren);
    if result != ResultCode::OK {
        println!("| HUB: failed to allocate new device");
        return result;
    }
    println!("Max6 children: {}", data.MaxChildren);

    result = HubPortGetStatus(device, port);
    if result != ResultCode::OK {
        println!("| HUB: failed to get status (3) for port {}", port + 1);
        return result;
    }

    println!("Max2 children: {}", data.MaxChildren);
    let child_dev = unsafe { &mut *(data.Children[port as usize]) };
    println!("| HUB: allocated new device {}", child_dev.number);
    let port_status = portStatus.Status;
    if port_status.contains(HubPortStatus::LowSpeedAttached) {
        child_dev.speed = UsbSpeed::Low;
    } else if port_status.contains(HubPortStatus::HighSpeedAttached) {
        child_dev.speed = UsbSpeed::High;
    } else {
        child_dev.speed = UsbSpeed::Full;
    }

    child_dev.parent = Some(device);
    child_dev.port_number = port;

    result = UsbAttachDevice(child_dev);
    if result != ResultCode::OK {
        println!("| HUB: Could not connect to new device");
        println!("| HUB: PORT CONNECTION CHANGED NOT IMPLEMENTED");
        return result;
    }

    return ResultCode::OK;
}

fn HubCheckConnection(device: &mut UsbDevice, port: u8) -> ResultCode {
    let mut data = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut HubDevice) };
    let mut result = HubPortGetStatus(device, port);

    println!("| HUB: HubCheckConnection for device {} port {}", device.number, port);

    if result != ResultCode::OK {
        println!("| HUB: failed to get status (1) for port {}", port + 1);
        return result;
    }

    let port_status = data.PortStatus[port as usize].Status;
    let port_change = data.PortStatus[port as usize].Change;

    println!("| HUB: port_status: {:#x}", port_status);
    println!("| HUB: port_change: {:#x}", port_change);

    println!("Max children: {}", data.MaxChildren);

    if port_change.contains(HubPortStatusChange::ConnectedChanged) {
        println!("| HUB: Port {} connected changed", port + 1);
        HubPortConnectionChanged(device, port);
    }

    if port_change.contains(HubPortStatusChange::EnabledChanged) {
        if HubChangePortFeature(device, HubPortFeature::FeatureEnableChange, port, false) != ResultCode::OK {
            println!("| HUB: failed to clear enable change for port {}", port + 1);
        }

        //This may indicate EM interference
        if !port_status.contains(HubPortStatus::Enabled) && port_status.contains(HubPortStatus::Connected) && !data.Children[port as usize].is_null() {
            println!("| HUB: Port {} enabled but not connected", port + 1);
            HubPortConnectionChanged(device, port);
        }
    }

    if port_status.contains(HubPortStatus::Suspended) {
        if HubChangePortFeature(device, HubPortFeature::FeatureSuspend, port, false) != ResultCode::OK {
            println!("| HUB: failed to clear suspend for port {}", port + 1);
        }
    }

    if port_change.contains(HubPortStatusChange::OverCurrentChanged) {
        if HubChangePortFeature(device, HubPortFeature::FeatureOverCurrent, port, false) != ResultCode::OK {
            println!("| HUB: failed to clear over current for port {}", port + 1);
        }
        HubPowerOn(device);
    }

    if port_change.contains(HubPortStatusChange::ResetChanged) {
        if HubChangePortFeature(device, HubPortFeature::FeatureResetChange, port, false) != ResultCode::OK {
            println!("| HUB: failed to clear reset change for port {}", port + 1);
        }
    }

    return ResultCode::OK;
}

fn HubCheckForChange(device: &mut UsbDevice) {
    let mut data = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut HubDevice) };

    println!("| HUB: HubCheckForChange for device {} children {}", device.number, data.MaxChildren);

    for i in 0..data.MaxChildren {
        if HubCheckConnection(device, i as u8) != ResultCode::OK {
            continue;
        }

        if !data.Children[i as usize].is_null() {
            if let Some(check_for_change) = device.device_check_for_change {
                check_for_change(unsafe { &mut *(data.Children[i as usize]) });
            }
        }
    }
}

fn HubCheckConnectionDevice(device: &mut UsbDevice, child: &mut UsbDevice) -> ResultCode {
    let mut data = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut HubDevice) };

    if child.parent == Some(device) && child.port_number >= 0 && child.port_number < data.MaxChildren as u8 && data.Children[child.port_number as usize] == child {
        let result = HubCheckConnection(device, child.port_number as u8);
        if result != ResultCode::OK {
            return result;
        }

        return if data.Children[child.port_number as usize] == child { ResultCode::OK } else { ResultCode::ErrorDevice };
    } else {
        return ResultCode::ErrorArgument;
    }
}


fn HubAttach(device: &mut UsbDevice, interface_number: u32) -> ResultCode {

    if device.interfaces[interface_number as usize].endpoint_count != 1 {
        println!("| HUB: cannot enumerate hub with {} endpoints", device.interfaces[interface_number as usize].endpoint_count);
        return ResultCode::ErrorIncompatible;
    }

    if device.endpoints[interface_number as usize][0].endpoint_address.Number >> 7 == 0 {
        println!("| HUB: cannot enumerate hub only one output endpoint {}", device.endpoints[interface_number as usize][0].endpoint_address.Number >> 7);
        return ResultCode::ErrorIncompatible;
    }

    if device.endpoints[interface_number as usize][0].attributes.Type & 0x3 != UsbTransfer::Interrupt as u8 {
        println!("| HUB: cannot enumerate hub without interrupt endpoint");
        return ResultCode::ErrorIncompatible;
    }

    println!("| HUB: HubAttach");

    //TOOD: Register the proper functions
    device.device_deallocate = None; //TODO: Implement
    device.device_detached = None; //TODO: Implement
    device.device_check_for_change = Some(HubCheckForChange);
    device.device_child_detached = None; //TODO: Implement
    device.device_child_reset = Some(HubChildReset);
    device.device_check_connection = Some(HubCheckConnectionDevice);


    let boxed = Box::new(HubDevice::new());
    let boxed_bytes = Box::into_raw(boxed);
    let byte_slice = unsafe { core::slice::from_raw_parts_mut(boxed_bytes as *mut u8, size_of::<HubDevice>()) };
    let byte_bytes = unsafe { Box::from_raw(byte_slice as *mut [u8]) };
    //TODO: I have no clue what I'm doing
    device.driver_data = Some(byte_bytes);
    

    let mut hub = unsafe { &mut *(device.driver_data.as_mut().unwrap().as_mut_ptr() as *mut HubDevice) };
    println!("| Hub Driver Data instantiated");
    println!("hub header {:#x}", &mut hub.Header as *mut UsbDriverDataHeader as usize);
    println!("hub header datasize {:#x}", &mut hub.Header.data_size as *const u32 as usize);

    let data_ize = hub.Header.data_size;
    println!("hub thing {}", data_ize  );

    hub.Header.data_size = size_of::<HubDevice>() as u32;
    hub.Header.device_driver = DeviceDriverHub;
    hub.Descriptor = None;
    
    for i in 0..MAX_CHILDREN_PER_DEVICE {
        hub.Children[i] = core::ptr::null_mut();
    }
    println!("| Hub Read Descriptor");
    let mut result = HubReadDescriptor(device);
    if result != ResultCode::OK {
        return result;
    }

    let mut HubDescriptor = hub.Descriptor.as_mut().unwrap().as_mut();

    if HubDescriptor.PortCount > MAX_CHILDREN_PER_DEVICE as u8 {
        println!("| HUB: hub is too big for this driver to handle");
        hub.MaxChildren = MAX_CHILDREN_PER_DEVICE as u32;
    } else {
        hub.MaxChildren = HubDescriptor.PortCount as u32;
    }

    //TODO: Hope HubDescriptor.Attributes is correct
    println!("| HUB: hub has {} children", hub.MaxChildren);


    result = HubGetStatus(device);
    if result != ResultCode::OK {
        println!("| HUB: failed to get hub status");
        return result;
    }

    let mut status = unsafe { &mut (hub.Status) };

    result = HubPowerOn(device);
    if result != ResultCode::OK {
        println!("| HUB: failed to power on hub");
        return result;
    }

    result = HubGetStatus(device);

    if result != ResultCode::OK {
        println!("| HUB: failed to get hub status");
        return result;
    }

    for port in 0..hub.MaxChildren {
        HubCheckConnection(device, port as u8);
    }

    return ResultCode::OK;

}