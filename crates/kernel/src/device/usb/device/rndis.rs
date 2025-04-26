/**
 *
 * usb/device/rndis.rs
 *  By Aaron Lo
 *   
 */
//RNDIS protocol implementation
//Based off of https://learn.microsoft.com/en-us/windows-hardware/drivers/network/remote-ndis-communication
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

const ControlTimeoutPeriod: u32 = 10;
#[allow(dead_code)]
const KeepAliveTimeoutPeriod: u32 = 5;

pub fn rndis_init(device: &mut UsbDevice) {
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
}

pub fn rndis_initialize_msg(device: &mut UsbDevice) -> ResultCode {
    let buffer = &mut RndisInitializeMsg {
        message_type: 0x00000002,
        message_length: size_of::<RndisInitializeMsg>() as u32,
        request_id: 0,
        major_version: 1,
        minor_version: 0,
        max_transfer_size: 8,
    };

    let mut buffer_req = [0u8; 52];

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
            buffer as *mut RndisInitializeMsg as *mut u8,
            size_of::<RndisInitializeMsg>() as u32,
            &mut UsbDeviceRequest {
                request_type: 0x21,
                request: convert_usb_device_request_cdc(
                    UsbDeviceRequestCDC::SendEncapsulatedCommand,
                ),
                value: 0,
                index: 0,
                length: size_of::<RndisInitializeMsg>() as u16,
            },
            ControlTimeoutPeriod,
        )
    };

    if result != ResultCode::OK {
        print!("| RNDIS: Failed to send initialize message.\n");
        return ResultCode::ErrorDevice;
    }

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
            buffer_req.as_mut_ptr(),
            size_of::<RndisInitializeMsgCmplt>() as u32,
            &mut UsbDeviceRequest {
                request_type: 0xA1,
                request: convert_usb_device_request_cdc(
                    UsbDeviceRequestCDC::GetEncapsulatedResponse,
                ),
                value: 0,
                index: 0,
                length: size_of::<RndisInitializeMsgCmplt>() as u16,
            },
            ControlTimeoutPeriod,
        )
    };

    if result != ResultCode::OK {
        print!("| RNDIS: Failed to receive initialize message.\n");
        return ResultCode::ErrorDevice;
    }

    // //TODO: check if the message is correct
    // //TODO: transfer this knowledge to the net module
    // let buffer_req32 = buffer_req.as_mut_ptr() as *mut u32;
    // println!("| RNDIS: Message Type: {:x}", unsafe { *buffer_req32 });
    // println!("| RNDIS: Message Length: {}", unsafe {
    //     *buffer_req32.add(1)
    // });
    // println!("| RNDIS: Request ID: {:x}", unsafe { *buffer_req32.add(2) });
    // println!("| RNDIS: Status: {:x}", unsafe { *buffer_req32.add(3) });
    // println!("| RNDIS: Major Version: {:x}", unsafe {
    //     *buffer_req32.add(4)
    // });
    // println!("| RNDIS: Minor Version: {:x}", unsafe {
    //     *buffer_req32.add(5)
    // });
    // println!("| RNDIS: Device Flags: {:x}", unsafe {
    //     *buffer_req32.add(6)
    // });
    // println!("| RNDIS: Medium: {:x}", unsafe { *buffer_req32.add(7) });
    // println!("| RNDIS: Max PacketsPerMessage: {:x}", unsafe {
    //     *buffer_req32.add(8)
    // });
    // println!("| RNDIS: Max TransferSize: {}", unsafe {
    //     *buffer_req32.add(9)
    // });
    // println!("| RNDIS: PacketAlignmentFactor: {:x}", unsafe {
    //     *buffer_req32.add(10)
    // });
    // println!("| RNDIS: AFListOffset: {:x}", unsafe {
    //     *buffer_req32.add(11)
    // });
    // println!("| RNDIS: AFListSize: {:x}", unsafe {
    //     *buffer_req32.add(12)
    // });

    return ResultCode::OK;
}

pub unsafe fn rndis_query_msg(
    device: &mut UsbDevice,
    oid: OID,
    buffer_req: *mut u8,
    buffer_length: u32,
) -> ResultCode {
    if buffer_length < size_of::<RndisQueryMsgCmplt>() as u32 {
        print!("| RNDIS: Buffer length is too small.\n");
        return ResultCode::ErrorArgument;
    }

    let query_msg = &mut RndisQueryMsg {
        message_type: 0x4,
        message_length: size_of::<RndisQueryMsg>() as u32,
        request_id: 1,
        oid,
        information_buffer_length: 0,
        information_buffer_offset: 0,
        device_vc_handle: 0,
    };

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
            query_msg as *mut RndisQueryMsg as *mut u8,
            size_of::<RndisQueryMsg>() as u32,
            &mut UsbDeviceRequest {
                request_type: 0x21,
                request: convert_usb_device_request_cdc(
                    UsbDeviceRequestCDC::SendEncapsulatedCommand,
                ),
                value: 0,
                index: 0,
                length: size_of::<RndisQueryMsg>() as u16,
            },
            ControlTimeoutPeriod,
        )
    };

    if result != ResultCode::OK {
        print!("| RNDIS: Failed to send query message.\n");
        return ResultCode::ErrorDevice;
    }

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
            buffer_req,
            buffer_length,
            &mut UsbDeviceRequest {
                request_type: 0xA1,
                request: convert_usb_device_request_cdc(
                    UsbDeviceRequestCDC::GetEncapsulatedResponse,
                ),
                value: 0,
                index: 0,
                length: buffer_length as u16,
            },
            ControlTimeoutPeriod,
        )
    };

    if result != ResultCode::OK {
        print!("| RNDIS: Failed to receive query message.\n");
        return ResultCode::ErrorDevice;
    }
    // println!("| RNDIS: Received query message.");

    // let buffer_req32 = buffer_req as *mut u32;
    // println!("| RNDIS: Message Type: {:x}", unsafe { *buffer_req32 });
    // println!("| RNDIS: Message Length: {}", unsafe {
    //     *buffer_req32.add(1)
    // });
    // println!("| RNDIS: Request ID: {:x}", unsafe { *buffer_req32.add(2) });
    // println!("| RNDIS: Status: {:x}", unsafe { *buffer_req32.add(3) });
    // println!("| RNDIS: Information Buffer Length: {}", unsafe {
    //     *buffer_req32.add(4)
    // });
    // println!("| RNDIS: Information Buffer Offset: {:x}", unsafe {
    //     *buffer_req32.add(5)
    // });
    // println!("| RNDIS: Start of buffer: {:x}", unsafe {
    //     *buffer_req32.add(6)
    // });

    // //start reading from request id + buffer offset to request id + buffer offset + buffer length
    // let buffer_length = unsafe { *buffer_req32.add(4) };
    // let buffer_offset = unsafe { *buffer_req32.add(5) };
    // let buffer = unsafe { buffer_req.offset(24) as *mut u8 };

    // println!("| RNDIS: Buffer Length: {}", buffer_length);
    // println!("| RNDIS: Buffer Offset: {:x}", buffer_offset);

    // for i in 0..buffer_length {
    //     let byte = unsafe { *buffer.offset(i as isize) };
    //     print!("{:x} ", byte);
    // }

    return ResultCode::OK;
}

//Technically the value could be more than 4 bytes, but we don't care for now
//TODO: Fix if relevant
pub fn rndis_set_msg(device: &mut UsbDevice, oid: OID, value: u32) {
    let set_msg = &mut RndisSetMsg {
        message_type: 0x00000005,
        message_length: size_of::<RndisSetMsg>() as u32,
        request_id: 2,
        oid,
        information_buffer_length: 4,
        information_buffer_offset: 20,
        device_vc_handle: 0,
        value,
    };

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
            set_msg as *mut RndisSetMsg as *mut u8,
            size_of::<RndisSetMsg>() as u32,
            &mut UsbDeviceRequest {
                request_type: 0x21,
                request: convert_usb_device_request_cdc(
                    UsbDeviceRequestCDC::SendEncapsulatedCommand,
                ),
                value: 0,
                index: 0,
                length: size_of::<RndisSetMsg>() as u16,
            },
            ControlTimeoutPeriod,
        )
    };

    if result != ResultCode::OK {
        print!("| RNDIS: Failed to send set message.\n");
    }

    let buffer = &mut RndisSetMsgCmplt::default();
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
            buffer as *mut RndisSetMsgCmplt as *mut u8,
            size_of::<RndisSetMsgCmplt>() as u32,
            &mut UsbDeviceRequest {
                request_type: 0xA1,
                request: convert_usb_device_request_cdc(
                    UsbDeviceRequestCDC::GetEncapsulatedResponse,
                ),
                value: 0,
                index: 0,
                length: size_of::<RndisSetMsgCmplt>() as u16,
            },
            ControlTimeoutPeriod,
        )
    };

    if result != ResultCode::OK {
        print!("| RNDIS: Failed to receive set message.\n");
    }

    // let msg_cmplt = unsafe { &*(buffer.as_mut_ptr() as *mut RndisSetMsgCmplt) };
    // let message_type = buffer.message_type;
    // let message_length = buffer.message_length;
    // let request_id = buffer.request_id;
    // let status = buffer.status;

    // println!("| RNDIS: Message Type: {:x}", message_type);
    // println!("| RNDIS: Message Length: {}", message_length);
    // println!("| RNDIS: Request ID: {:x}", request_id);
    // println!("| RNDIS: Status: {:#?}", status);
}

pub unsafe fn rndis_send_packet(
    device: &mut UsbDevice,
    buffer: *mut u8,
    buffer_length: u32,
) -> ResultCode {
    let size = size_of::<RndisPacketMsg>() as u32 + buffer_length;

    let buffer_req = &mut RndisPacketMsg {
        message_type: 0x1,
        message_length: size,
        data_offset: size_of::<RndisPacketMsg>() as u32 - 8,
        data_length: buffer_length,
        oob_data_offset: 0,
        oob_data_length: 0,
        num_oob_data_elements: 0,
        per_packet_info_offset: 0,
        per_packet_info_length: 0,
        vc_handle: 0,
        reserved: 0,
    };

    let mut complete_buffer = vec![0u8; size as usize];

    //copy buffer_req to complete_buffer
    unsafe {
        core::ptr::copy_nonoverlapping(
            buffer_req as *mut RndisPacketMsg as *mut u8,
            complete_buffer.as_mut_ptr(),
            size_of::<RndisPacketMsg>() as usize,
        );
        core::ptr::copy_nonoverlapping(
            buffer,
            complete_buffer
                .as_mut_ptr()
                .offset(size_of::<RndisPacketMsg>() as isize),
            buffer_length as usize,
        );
    }

    let result = unsafe {
        UsbSendBulkMessage(
            device,
            UsbPipeAddress {
                transfer_type: UsbTransfer::Bulk,
                speed: device.speed,
                end_point: 2,
                device: device.number as u8,
                direction: UsbDirection::Out,
                max_size: size_from_number(64),
                _reserved: 0,
            },
            complete_buffer.into_boxed_slice(),
            size as u32,
            PacketId::Data0,
            1,
            10,
        )
    };

    if result != ResultCode::OK {
        print!("| RNDIS: Failed to send packet message.\n");
        return result;
    }

    return ResultCode::OK;
}

pub unsafe fn rndis_receive_packet(
    device: &mut UsbDevice,
    buffer: Box<[u8]>,
    buffer_length: u32,
) -> ResultCode {
    let result = unsafe {
        UsbSendBulkMessage(
            device,
            UsbPipeAddress {
                transfer_type: UsbTransfer::Bulk,
                speed: device.speed,
                end_point: 2,
                device: device.number as u8,
                direction: UsbDirection::In,
                max_size: size_from_number(64),
                _reserved: 0,
            },
            buffer,
            buffer_length,
            PacketId::Data0,
            2,
            10,
        )
    };

    if result != ResultCode::OK {
        print!("| RNDIS: Failed to receive packet message.\n");
        return result;
    }

    return ResultCode::OK;
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct RndisInitializeMsg {
    pub message_type: u32,
    pub message_length: u32,
    pub request_id: u32,
    pub major_version: u32,
    pub minor_version: u32,
    pub max_transfer_size: u32,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct RndisInitializeMsgCmplt {
    pub message_type: u32,
    pub message_length: u32,
    pub request_id: u32,
    pub status: RndisStatusValue,
    pub major_version: u32,
    pub minor_version: u32,
    pub device_flags: u32,
    pub medium: u32,
    pub max_packets_per_message: u32,
    pub max_transfer_size: u32,
    pub packet_alignment_factor: u32,
    pub af_list_offset: u32,
    pub af_list_size: u32,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct RndisQueryMsg {
    pub message_type: u32,
    pub message_length: u32,
    pub request_id: u32,
    pub oid: OID,
    pub information_buffer_length: u32,
    pub information_buffer_offset: u32,
    pub device_vc_handle: u32,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct RndisQueryMsgCmplt {
    pub message_type: u32,
    pub message_length: u32,
    pub request_id: u32,
    pub status: RndisStatusValue,
    pub information_buffer_length: u32,
    pub information_buffer_offset: u32,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct RndisSetMsg {
    pub message_type: u32,
    pub message_length: u32,
    pub request_id: u32,
    pub oid: OID,
    pub information_buffer_length: u32,
    pub information_buffer_offset: u32,
    pub device_vc_handle: u32,
    pub value: u32,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct RndisSetMsgCmplt {
    pub message_type: u32,
    pub message_length: u32,
    pub request_id: u32,
    pub status: RndisStatusValue,
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct RndisPacketMsg {
    pub message_type: u32,
    pub message_length: u32,
    pub data_offset: u32,
    pub data_length: u32,
    pub oob_data_offset: u32,
    pub oob_data_length: u32,
    pub num_oob_data_elements: u32,
    pub per_packet_info_offset: u32,
    pub per_packet_info_length: u32,
    pub vc_handle: u32,
    pub reserved: u32,
}

#[repr(u32)]
#[derive(Default, Debug, Clone, Copy)]
pub enum RndisStatusValue {
    #[default]
    RNDIS_STATUS_SUCCESS = 0x00000000, /* Success */
    RNDIS_STATUS_FAILURE = 0xc0000001,       /* Unspecified error */
    RNDIS_STATUS_INVALID_DATA = 0xc0010015,  /* Invalid data */
    RNDIS_STATUS_NOT_SUPPORTED = 0xc00000bb, /* Unsupported request */
    RNDIS_STATUS_MEDIA_CONNECT = 0x4001000b, /* Device connected */
    RNDIS_STATUS_MEDIA_DISCONNECT = 0x4001000c, /* Device disconnected */
}

#[repr(u32)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OID {
    /* Required Object IDs (OIDs) */
    #[default]
    OID_GEN_SUPPORTED_LIST = 0x00010101,
    OID_GEN_HARDWARE_STATUS = 0x00010102,
    OID_GEN_MEDIA_SUPPORTED = 0x00010103,
    OID_GEN_MEDIA_IN_USE = 0x00010104,
    // OID_GEN_MAXIMUM_LOOKAHEAD = 0x00010105,
    OID_GEN_MAXIMUM_FRAME_SIZE = 0x00010106,
    OID_GEN_LINK_SPEED = 0x00010107,
    // OID_GEN_TRANSMIT_BUFFER_SPACE = 0x00010108,
    // OID_GEN_RECEIVE_BUFFER_SPACE = 0x00010109,
    OID_GEN_TRANSMIT_BLOCK_SIZE = 0x0001010A,
    OID_GEN_RECEIVE_BLOCK_SIZE = 0x0001010B,
    OID_GEN_VENDOR_ID = 0x0001010C,
    OID_GEN_VENDOR_DESCRIPTION = 0x0001010D,
    OID_GEN_CURRENT_PACKET_FILTER = 0x0001010E,
    // OID_GEN_CURRENT_LOOKAHEAD = 0x0001010F,
    // OID_GEN_DRIVER_VERSION = 0x00010110,
    OID_GEN_MAXIMUM_TOTAL_SIZE = 0x00010111,
    // OID_GEN_PROTOCOL_OPTIONS = 0x00010112,
    // OID_GEN_MAC_OPTIONS = 0x00010113,
    OID_GEN_MEDIA_CONNECT_STATUS = 0x00010114,
    // OID_GEN_MAXIMUM_SEND_PACKETS = 0x00010115,

    /* Optional OIDs */
    // OID_GEN_MEDIA_CAPABILITIES          = 0x00010201,
    OID_GEN_PHYSICAL_MEDIUM = 0x00010202,

    /* Required statistics OIDs */
    // TODO
    /* Optional statistics OIDs */
    // TODO

    /* IEEE 802.3 (Ethernet) OIDs */
    OID_802_3_PERMANENT_ADDRESS = 0x01010101,
    OID_802_3_CURRENT_ADDRESS = 0x01010102,
    OID_802_3_MULTICAST_LIST = 0x01010103,
    OID_802_3_MAXIMUM_LIST_SIZE = 0x01010104,
    OID_802_3_MAC_OPTIONS = 0x01010105,
    OID_802_3_RCV_ERROR_ALIGNMENT = 0x01020101,
    OID_802_3_XMIT_ONE_COLLISION = 0x01020102,
    OID_802_3_XMIT_MORE_COLLISIONS = 0x01020103,
    // OID_802_3_XMIT_DEFERRED             = 0x01020201,
    // OID_802_3_XMIT_MAX_COLLISIONS       = 0x01020202,
    // OID_802_3_RCV_OVERRUN               = 0x01020203,
    // OID_802_3_XMIT_UNDERRUN             = 0x01020204,
    // OID_802_3_XMIT_HEARTBEAT_FAILURE    = 0x01020205,
    // OID_802_3_XMIT_TIMES_CRS_LOST       = 0x01020206,
    // OID_802_3_XMIT_LATE_COLLISIONS      = 0x01020207,
}
