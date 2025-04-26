use crate::device::usb::types::*;
use crate::device::usb::usbd::device::*;
use crate::device::usb::usbd::pipe::*;
use crate::device::usb::usbd::request::*;
use crate::device::usb::usbd::usbd::UsbSendBulkMessage;
use crate::device::usb::UsbControlMessage;

use crate::device::usb::device::net::*;
use crate::device::usb::PacketId;
use alloc::boxed::Box;
use alloc::vec;

/**
 *
 * usb/device/ax88179.rs
 *  By Aaron Lo
 *   Based off the freeBSD driver if_axge.c
 */

fn axge_read_cmd1(device: &mut UsbDevice, cmd: u8, reg: u16) -> u8 {
    let val: u8 = 0;

    axge_write_mem(device, cmd, 1, reg, &val as *const u8 as *mut u8, 1);

    return val;
}

fn axge_read_cmd2(device: &mut UsbDevice, cmd: u8, reg: u16) -> u16 {
    let val: u16 = 0;

    axge_write_mem(device, cmd, 2, reg, &val as *const u16 as *mut u8, 2);

    return val;
}

fn axge_write_cmd1(device: &mut UsbDevice, cmd: u8, reg: u16, val: u8) {
    axge_write_mem(device, cmd, 1, reg, &val as *const u8 as *mut u8, 1);
}

fn axge_write_cmd2(device: &mut UsbDevice, cmd: u8, reg: u16, val: u16) {
    axge_write_mem(device, cmd, 2, reg, &val as *const u16 as *mut u8, 2);
}


fn axge_write_mem(device: &mut UsbDevice, cmd: u8, index: u16, val: u16, buf: *mut u8, len: u32) -> ResultCode {

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
            buf,
            len,
            &mut UsbDeviceRequest {
                request_type: 0x40,
                request: command_to_usb_device_request(cmd),
                index: index as u16,
                value: val as u16,
                length: len as u16,
            },
            1000, // timeout
        )
    };

    if result != ResultCode::OK {
        print!("| AXGE: Failed to write memory.\n");
        return result;
    }

    return ResultCode::OK;
}