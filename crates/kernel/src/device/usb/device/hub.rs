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

use super::super::usbd::descriptors::*;
use super::super::usbd::device::*;
use super::super::usbd::usbd::*;

use crate::device::system_timer::micro_delay;
use crate::device::usb::hcd::hub::*;
use crate::device::usb::types::*;
use crate::device::usb::usbd::pipe::*;
use crate::device::usb::usbd::request::*;
use alloc::boxed::Box;

pub fn HubLoad(bus: &mut UsbBus) {
    bus.interface_class_attach[InterfaceClass::InterfaceClassHub as usize] = Some(HubAttach);
}

fn HubReadDescriptor(device: &mut UsbDevice) -> ResultCode {
    let mut header = UsbDescriptorHeader::default();

    let mut result = unsafe {
        UsbGetDescriptor(
            device,
            DescriptorType::Hub,
            0,
            0,
            &mut header as *mut UsbDescriptorHeader as *mut u8,
            size_of::<UsbDescriptorHeader>() as u32,
            size_of::<UsbDescriptorHeader>() as u32,
            0x20,
        )
    };

    if result != ResultCode::OK {
        println!("| HUB: failed to read descriptor");
        return result;
    }

    let hub = device.driver_data.downcast::<HubDevice>().unwrap();
    if hub.Descriptor.is_none() {
        // println!(
        //     "| HUB: allocating descriptor of size {} with HubDescriptor {}",
        //     header.descriptor_length,
        //     size_of::<HubDescriptor>()
        // );
        hub.Descriptor = Some(Box::new(HubDescriptor::default()));

        //TODO: Update this creation as well
    }

    // TODO: this is still UB b/c descriptor aliases parts of device, but it's fine for now
    let descriptor = hub.Descriptor.as_mut().unwrap().as_mut() as *mut HubDescriptor as *mut u8;
    result = unsafe {
        UsbGetDescriptor(
            device,
            DescriptorType::Hub,
            0,
            0,
            descriptor,
            header.descriptor_length as u32,
            header.descriptor_length as u32,
            0x20,
        )
    };
    if result != ResultCode::OK {
        println!("| HUB: failed to read full descriptor");
        return result;
    }

    return ResultCode::OK;
}

fn HubGetStatus(device: &mut UsbDevice) -> ResultCode {
    let hub = device.driver_data.downcast::<HubDevice>().unwrap();
    let status = (&mut hub.Status as *mut HubFullStatus) as *mut u8;
    let result = unsafe {
        UsbControlMessage(
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
            ControlMessageTimeout as u32,
        )
    };

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

fn HubChangePortFeature(
    device: &mut UsbDevice,
    feature: HubPortFeature,
    port: u8,
    set: bool,
) -> ResultCode {
    let result = unsafe {
        UsbControlMessage(
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
                request: if set {
                    UsbDeviceRequestRequest::SetFeature
                } else {
                    UsbDeviceRequestRequest::ClearFeature
                },
                length: 0,
                value: feature as u16,
                index: (port + 1) as u16,
            },
            ControlMessageTimeout as u32,
        )
    };

    return result;
}

fn HubPowerOn(device: &mut UsbDevice) -> ResultCode {
    let hub = device.driver_data.downcast::<HubDevice>().unwrap();
    let hub_desc = hub.Descriptor.as_mut().unwrap().as_mut(); //unsafe { &mut *(.as_mut_ptr() as *mut HubDescriptor) };
    let max_children = hub.MaxChildren;
    let mut delay = hub_desc.PowerGoodDelay as u32;

    for i in 0..max_children {
        if HubChangePortFeature(device, HubPortFeature::FeaturePower, i as u8, true)
            != ResultCode::OK
        {
            println!("| HUB: failed to power on port {}", i);
        }
    }

    if delay == 0 {
        delay = 50; //100 ms
    }

    println!("| HUB: powering on hub, waiting for {}ms", 2 * delay);
    micro_delay(delay * 2000);

    return ResultCode::OK;
}

fn HubPortGetStatus(device: &mut UsbDevice, port: u8) -> ResultCode {
    let hub = device.driver_data.downcast::<HubDevice>().unwrap();
    let port_status = &mut hub.PortStatus[port as usize] as *mut HubPortFullStatus as *mut u8;
    let result = unsafe {
        UsbControlMessage(
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
            },
            ControlMessageTimeout as u32,
        )
    };

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
    let mut result;
    let mut retry_max = 0;
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

            let data = device.driver_data.downcast::<HubDevice>().unwrap();
            let status = &data.PortStatus[port as usize];
            let port_changed = status.Change;
            let port_status = status.Status;
            if port_changed.contains(HubPortStatusChange::ResetChanged)
                || port_status.contains(HubPortStatus::Enabled)
            {
                break;
            }
        }

        if timeout == 10 {
            continue;
        }

        let data = device.driver_data.downcast::<HubDevice>().unwrap();
        let status = &data.PortStatus[port as usize];
        let port_change = status.Change;
        let port_status = status.Status;
        if port_change.contains(HubPortStatusChange::ConnectedChanged)
            || !port_status.contains(HubPortStatus::Connected)
        {
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

    result = HubChangePortFeature(device, HubPortFeature::FeatureResetChange, port, false);
    if result != ResultCode::OK {
        println!("| HUB: failed to clear reset change for port {}", port + 1);
    }

    return ResultCode::OK;
}

fn HubChildReset(device: &mut UsbDevice, child: &mut UsbDevice) -> ResultCode {
    let device_ptr = device as *mut _;
    let data = device.driver_data.downcast::<HubDevice>().unwrap();
    if child.parent == Some(device_ptr)
        && child.port_number < data.MaxChildren as u8
        && data.Children[child.port_number as usize] == child
    {
        return HubPortReset(device, child.port_number);
    } else {
        println!("| HUB: child reset failed");
        return ResultCode::ErrorArgument;
    }
}

fn HubPortConnectionChanged(device: &mut UsbDevice, port: u8) -> ResultCode {
    let mut result = HubPortGetStatus(device, port);

    if result != ResultCode::OK {
        println!("| HUB: failed to get status (2) for port {}", port + 1);
        return result;
    }

    println!("| HUB: port {} status: {:#x}", port + 1, 10);

    result = HubChangePortFeature(device, HubPortFeature::FeatureConnectionChange, port, false);

    if result != ResultCode::OK {
        println!(
            "| HUB: failed to clear connection change for port {}",
            port + 1
        );
        return result;
    }
    let data = device.driver_data.downcast::<HubDevice>().unwrap();
    let port_status = data.PortStatus[port as usize].Status;
    if (!(port_status.contains(HubPortStatus::Connected))
        && !(port_status.contains(HubPortStatus::Enabled)))
        || !data.Children[port as usize].is_null()
    {
        println!("| HUB: Disconnected");

        println!("| HUB: PORT CONNECTION CHANGED NOT IMPLEMENTED");
        return ResultCode::ErrorIncompatible;
    }

    result = HubPortReset(device, port);
    if result != ResultCode::OK {
        println!("| HUB: count not reset port {} for new device", port + 1);
        return result;
    }

    let data = device.driver_data.downcast::<HubDevice>().unwrap();
    let mut dev = unsafe { Box::new(UsbDevice::new(device.bus, 0)) };
    data.Children[port as usize] = dev.as_mut() as *mut UsbDevice;
    result = UsbAllocateDevice(dev);
    if result != ResultCode::OK {
        println!("| HUB: failed to allocate new device");
        return result;
    }

    result = HubPortGetStatus(device, port);
    if result != ResultCode::OK {
        println!("| HUB: failed to get status (3) for port {}", port + 1);
        return result;
    }

    let data = device.driver_data.downcast::<HubDevice>().unwrap();
    let child_dev = unsafe { &mut *(data.Children[port as usize]) };
    println!("| HUB: allocated new device {}", child_dev.number);
    let port_status = data.PortStatus[port as usize].Status;
    if port_status.contains(HubPortStatus::LowSpeedAttached) {
        child_dev.speed = UsbSpeed::Low;
    } else if port_status.contains(HubPortStatus::HighSpeedAttached) {
        child_dev.speed = UsbSpeed::High;
    } else {
        child_dev.speed = UsbSpeed::Full;
    }

    println!("| HUB: new device speed {:?}", child_dev.speed);

    child_dev.parent = Some(device);
    child_dev.port_number = port;

    println!("| HUB: attach device {}", child_dev.number);
    result = UsbAttachDevice(child_dev);
    if result != ResultCode::OK {
        println!("| HUB: Could not connect to new device");
        println!("| HUB: PORT CONNECTION CHANGED NOT IMPLEMENTED");
        return result;
    }

    return ResultCode::OK;
}

fn HubCheckConnection(device: &mut UsbDevice, port: u8) -> ResultCode {
    let data = device.driver_data.downcast::<HubDevice>().unwrap();
    let prevHubStatus = data.PortStatus[port as usize].Status;
    let prevConnected = prevHubStatus.contains(HubPortStatus::Connected);
    let result = HubPortGetStatus(device, port);

    println!(
        "| HUB: HubCheckConnection for device {} port {}",
        device.number, port
    );

    if result != ResultCode::OK {
        println!("| HUB: failed to get status (1) for port {}", port + 1);
        return result;
    }

    let data = device.driver_data.downcast::<HubDevice>().unwrap();
    let port_status = data.PortStatus[port as usize].Status;
    let mut port_change = data.PortStatus[port as usize].Change;

    if device.number == 1 {
        if prevConnected != port_status.contains(HubPortStatus::Connected) {
            port_change.insert(HubPortStatusChange::ConnectedChanged);
        }
    }

    if port_change.contains(HubPortStatusChange::ConnectedChanged) {
        println!("| HUB: Port {} connected changed", port + 1);
        HubPortConnectionChanged(device, port);
    }

    if port_change.contains(HubPortStatusChange::EnabledChanged) {
        if HubChangePortFeature(device, HubPortFeature::FeatureEnableChange, port, false)
            != ResultCode::OK
        {
            println!("| HUB: failed to clear enable change for port {}", port + 1);
        }

        let data = device.driver_data.downcast::<HubDevice>().unwrap();
        //This may indicate EM interference
        if !port_status.contains(HubPortStatus::Enabled)
            && port_status.contains(HubPortStatus::Connected)
            && !data.Children[port as usize].is_null()
        {
            println!("| HUB: Port {} enabled but not connected", port + 1);
            HubPortConnectionChanged(device, port);
        }
    }

    if port_status.contains(HubPortStatus::Suspended) {
        if HubChangePortFeature(device, HubPortFeature::FeatureSuspend, port, false)
            != ResultCode::OK
        {
            println!("| HUB: failed to clear suspend for port {}", port + 1);
        }
    }

    if port_change.contains(HubPortStatusChange::OverCurrentChanged) {
        if HubChangePortFeature(device, HubPortFeature::FeatureOverCurrent, port, false)
            != ResultCode::OK
        {
            println!("| HUB: failed to clear over current for port {}", port + 1);
        }
        HubPowerOn(device);
    }

    if port_change.contains(HubPortStatusChange::ResetChanged) {
        if HubChangePortFeature(device, HubPortFeature::FeatureResetChange, port, false)
            != ResultCode::OK
        {
            println!("| HUB: failed to clear reset change for port {}", port + 1);
        }
    }

    return ResultCode::OK;
}

fn HubCheckForChange(device: &mut UsbDevice) {
    let data = device.driver_data.downcast::<HubDevice>().unwrap();

    // TODO: THIS IS UNSOUND, AND WILL CAUSE ISSUES
    // (fixing this will likely require restructuring the UsbDevice struct and a lot of refactoring)
    let data = unsafe { &mut *(data as *mut HubDevice) };

    println!(
        "| HUB: HubCheckForChange for device {} children {}",
        device.number, data.MaxChildren
    );

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
    let device_ptr = device as *mut UsbDevice;
    let data = device.driver_data.downcast::<HubDevice>().unwrap();

    if child.parent == Some(device_ptr)
        && child.port_number < data.MaxChildren as u8
        && data.Children[child.port_number as usize] == child
    {
        let result = HubCheckConnection(device, child.port_number as u8);
        if result != ResultCode::OK {
            return result;
        }

        let data = device.driver_data.downcast::<HubDevice>().unwrap();
        return if data.Children[child.port_number as usize] == child {
            ResultCode::OK
        } else {
            ResultCode::ErrorDevice
        };
    } else {
        return ResultCode::ErrorArgument;
    }
}

fn HubAttach(device: &mut UsbDevice, interface_number: u32) -> ResultCode {
    if device.interfaces[interface_number as usize].endpoint_count != 1 {
        println!(
            "| HUB: cannot enumerate hub with {} endpoints",
            device.interfaces[interface_number as usize].endpoint_count
        );
        return ResultCode::ErrorIncompatible;
    }

    if device.endpoints[interface_number as usize][0]
        .endpoint_address
        .Number
        >> 7
        == 0
    {
        println!(
            "| HUB: cannot enumerate hub only one output endpoint {}",
            device.endpoints[interface_number as usize][0]
                .endpoint_address
                .Number
                >> 7
        );
        return ResultCode::ErrorIncompatible;
    }

    if device.endpoints[interface_number as usize][0]
        .attributes
        .Type
        & 0x3
        != UsbTransfer::Interrupt as u8
    {
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
    device.driver_data = DriverData::new(boxed);

    let hub = device.driver_data.downcast::<HubDevice>().unwrap();
    // println!("| HUB: Driver Data instantiated");

    hub.Header.data_size = size_of::<HubDevice>() as u32;
    hub.Header.device_driver = DeviceDriverHub;
    hub.Descriptor = None;

    for i in 0..MAX_CHILDREN_PER_DEVICE {
        hub.Children[i] = core::ptr::null_mut();
    }
    println!("| Hub Read Descriptor");
    let mut result = HubReadDescriptor(device);
    println!("| Hub Read Descriptor done: {:?}", result);
    if result != ResultCode::OK {
        return result;
    }

    let hub = device.driver_data.downcast::<HubDevice>().unwrap();
    let HubDescriptor = hub.Descriptor.as_mut().unwrap().as_mut();

    if HubDescriptor.PortCount > MAX_CHILDREN_PER_DEVICE as u8 {
        println!("| HUB: hub is too big for this driver to handle");
        hub.MaxChildren = MAX_CHILDREN_PER_DEVICE as u32;
    } else {
        hub.MaxChildren = HubDescriptor.PortCount as u32;
    }

    //TODO: Hope HubDescriptor.Attributes is correct
    // println!("| HUB: hub has {} children", hub.MaxChildren);

    result = HubGetStatus(device);
    if result != ResultCode::OK {
        println!("| HUB: failed to get hub status");
        return result;
    }

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

    let hub = device.driver_data.downcast::<HubDevice>().unwrap();

    for port in 0..hub.MaxChildren {
        HubCheckConnection(device, port as u8);
    }

    return ResultCode::OK;
}
