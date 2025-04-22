/******************************************************************************
*	hcd/dwc/roothub.c
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*
*	hcd/dwc/roothub.c contains code to control the DesignWare® Hi-Speed USB 2.0
*	On-The-Go (HS OTG) Controller's virtual root hub. The physical USB
*	connection to the computer is treated as a virtual 1 port USB hub for
*	simplicity, allowing the USBD to control it directly with a Hub driver.
*
*	THIS SOFTWARE IS NOT AFFILIATED WITH NOR ENDORSED BY SYNOPSYS IP.
******************************************************************************/

use super::dwc_otg::*;
use super::dwc_otgreg::*;
use crate::device::system_timer::micro_delay;
use crate::device::usb::hcd::hub::HubDescriptor;
use crate::device::usb::hcd::hub::*;
use crate::device::usb::types::*;
use crate::device::usb::usbd::descriptors::*;
use crate::device::usb::usbd::device::*;
use crate::device::usb::usbd::pipe::*;
use crate::device::usb::usbd::request::*;
use crate::sync::init;

use core::cmp::min;
use core::mem::size_of;
use core::ptr::copy;

pub unsafe fn memory_copy(dest: *mut u8, src: *const u8, len: usize) {
    if len == 0 {
        return;
    }

    unsafe {
        copy(src, dest, len); // Handles overlapping memory correctly
    }
}

pub unsafe fn memory_copy_buf(dest: &mut [u8; 1024], src: *const u8, len: usize) {
    if len == 0 {
        return;
    }

    unsafe {
        copy(src, dest.as_mut_ptr(), len); // Handles overlapping memory correctly
    }
}

const DeviceDescriptor: UsbDeviceDescriptor = UsbDeviceDescriptor {
    descriptor_length: 0x12,
    descriptor_type: DescriptorType::Device,
    usb_version: 0x0200,
    class: DeviceClass::DeviceClassHub,
    subclass: 0,
    protocol: 0,
    max_packet_size0: 64,
    vendor_id: 0,
    product_id: 0,
    version: 0x0100,
    manufacturer: 1,
    product: 2,
    serial_number: 0,
    configuration_count: 1,
};

#[repr(C, packed)]
struct ConfigurationDescriptor {
    configuration: UsbConfigurationDescriptor,
    interface: UsbInterfaceDescriptor,
    endpoint: UsbEndpointDescriptor,
}

const CONFIGURATION_DESCRIPTOR: ConfigurationDescriptor = ConfigurationDescriptor {
    configuration: UsbConfigurationDescriptor {
        descriptor_length: 9,
        descriptor_type: DescriptorType::Configuration,
        total_length: 0x19,
        interface_count: 1,
        configuration_value: 1,
        string_index: 0,
        attributes: UsbConfigurationAttributes {
            attributes: (1 << 6) | (1 << 7),
        },
        maximum_power: 0,
    },
    interface: UsbInterfaceDescriptor {
        descriptor_length: 9,
        descriptor_type: DescriptorType::Interface,
        number: 0,
        alternate_setting: 0,
        endpoint_count: 1,
        class: InterfaceClass::InterfaceClassHub,
        subclass: 0,
        protocol: 0,
        string_index: 0,
    },
    endpoint: UsbEndpointDescriptor {
        descriptor_length: 7,
        descriptor_type: DescriptorType::Endpoint,
        endpoint_address: UsbEndpointAddress {
            Number: 1 | (1 << 7),
        },
        attributes: UsbEndpointAttributes { Type: 3 },
        packet: UsbPacket { MaxSize: 8 },
        interval: 0xff,
    },
};

#[allow(dead_code)]
const STRING_0: UsbStringDescriptor = UsbStringDescriptor {
    descriptor_length: 4,
    descriptor_type: DescriptorType::String,
    data: [0x0409],
};

// const STRING_1: UsbStringDescriptor = UsbStringDescriptor {
//     DescriptorLength: (size_of::<[u16; 16]>() + 2) as u8,
//     DescriptorType: String,
//     Data: &['U' as u16, 'S' as u16, 'B' as u16, ' ' as u16, '2' as u16, '.' as u16, '0' as u16, ' ' as u16, 'R' as u16, 'o' as u16, 'o' as u16, 't' as u16, ' ' as u16, 'H' as u16, 'u' as u16, 'b' as u16],
// };

const HUB_DESCRIPTOR: HubDescriptor = HubDescriptor {
    DescriptorLength: 0x9,
    DescriptorType: DescriptorType::Hub,
    PortCount: 1,
    Attributes: 0,
    PowerGoodDelay: 50,
    MaximumHubPower: 0,
    Data: [0x01, 0xff],
};

pub unsafe fn HcdProcessRootHubMessage(
    device: &mut UsbDevice,
    pipe: UsbPipeAddress,
    buffer: *mut u8,
    buffer_length: u32,
    request: &mut UsbDeviceRequest,
) -> ResultCode {
    let mut result = ResultCode::OK;
    let mut reply_length = 0;
    device.error = UsbTransferError::Processing;

    if pipe.transfer_type == UsbTransfer::Interrupt {
        println!("| HCD.Hub: RootHub does not support IRQ pipes.");
        device.error = UsbTransferError::Stall;
        return ResultCode::OK;
    }

    match request.request {
        UsbDeviceRequestRequest::GetStatus => {
            match request.request_type {
                0x80 => {
                    unsafe {
                        *(buffer as *mut u16) = 1;
                    }
                    reply_length = 2;
                }
                0x81 | 0x82 => unsafe {
                    *(buffer as *mut u16) = 0;
                    reply_length = 2;
                },
                0xa0 => unsafe {
                    *(buffer as *mut u32) = 0;
                    reply_length = 4;
                },
                0xa3 => {
                    unsafe {
                        let hprt = read_volatile(DOTG_HPRT);
                        *(buffer as *mut u32) = 0;

                        let stat_buff = buffer as *mut HubPortFullStatus;

                        let mut status = 0;
                        status |= (hprt & HPRT_PRTCONNSTS) << 0;
                        status |= ((hprt & HPRT_PRTENA) >> 2) << 1;
                        status |= ((hprt & HPRT_PRTSUSP) >> 7) << 2;
                        status |= ((hprt & HPRT_PRTOVRCURRACT) >> 4) << 3;
                        status |= ((hprt & HPRT_PRTRST) >> 8) << 4;
                        status |= ((hprt & HPRT_PRTPWR) >> 12) << 8;

                        if ((hprt & HPRT_PRTSPD_MASK) >> HPRT_PRTSPD_SHIFT)
                            == HPRT_PRTSPD_HIGH as u32
                        {
                            status |= 1 << 10;
                        } else if ((hprt & HPRT_PRTSPD_MASK) >> HPRT_PRTSPD_SHIFT)
                            == HPRT_PRTSPD_LOW as u32
                        {
                            status |= 1 << 9;
                        }
                        status |=
                            ((hprt & (1 << HPRT_PRTTSTCTL_SHIFT)) >> HPRT_PRTTSTCTL_SHIFT) << 11;

                        let mut change = 0;
                        change |= ((hprt & HPRT_PRTCONNDET) >> 1) << 0;
                        change |= ((hprt & HPRT_PRTENCHNG) >> 3) << 1;
                        change |= ((hprt & HPRT_PRTOVRCURRCHNG) >> 5) << 3;
                        change |= 1 << 4;
                        //Don't even ask about this code, I hope its right
                        // println!(
                        //     "| HCD.Hub: HPRT: {:#x} Status: {:#x} Change: {:#x}",
                        //     hprt, status, change
                        // );
                        (*stat_buff).Status = HubPortStatus::from_bits_truncate(status as u16);
                        (*stat_buff).Change =
                            HubPortStatusChange::from_bits_truncate(change as u16);
                        reply_length = 4;
                    }
                }
                _ => {
                    device.error = UsbTransferError::Stall;
                }
            }
        }
        UsbDeviceRequestRequest::ClearFeature => {
            match request.request_type {
                0x2 | 0x20 => {}
                0x23 => {
                    match request.value {
                        1 => {
                            //FeatureEnable
                            let mut hprt = read_volatile(DOTG_HPRT);
                            hprt |= HPRT_PRTENA;
                            write_volatile(DOTG_HPRT, hprt & (0x1f140 | 0x4));
                        }
                        2 => {
                            //FeatureSuspend
                            write_volatile(DOTG_PCGCCTL, 0);
                            micro_delay(5000);
                            let mut hprt = read_volatile(DOTG_HPRT);
                            hprt |= HPRT_PRTRES;

                            write_volatile(DOTG_HPRT, hprt & (0x1f140 | 0x40));
                            micro_delay(100000);
                            hprt &= !HPRT_PRTRES;
                            hprt &= !HPRT_PRTSUSP;
                            write_volatile(DOTG_HPRT, hprt & (0x1f140 | 0xc0));
                        }
                        8 => {
                            //FeaturePower
                            let mut hprt = read_volatile(DOTG_HPRT);
                            hprt &= !HPRT_PRTPWR;
                            write_volatile(DOTG_HPRT, hprt & (0x1f140 | 0x1000));
                        }
                        16 => {
                            //FeatureConnectionChange
                            let mut hprt = read_volatile(DOTG_HPRT);
                            hprt |= HPRT_PRTCONNDET;
                            write_volatile(DOTG_HPRT, hprt & (0x1f140 | 0x2));
                        }
                        17 => {
                            //FeatureEnableChange
                            let mut hprt = read_volatile(DOTG_HPRT);
                            hprt |= HPRT_PRTENCHNG;
                            write_volatile(DOTG_HPRT, hprt & (0x1f140 | 0x8));
                        }
                        19 => {
                            //FeatureOverCurrentChange
                            let mut hprt = read_volatile(DOTG_HPRT);
                            hprt |= HPRT_PRTOVRCURRCHNG;
                            write_volatile(DOTG_HPRT, hprt & (0x1f140 | 0x20));
                        }
                        _ => {}
                    }
                }
                _ => {
                    result = ResultCode::ErrorArgument;
                }
            }
        }
        UsbDeviceRequestRequest::SetFeature => {
            match request.request_type {
                0x20 => {}
                0x23 => {
                    match request.value {
                        4 => {
                            println!("Roothub Exec: Port Reset");
                            
                            let hprt = read_volatile(DOTG_HPRT);
                            write_volatile(DOTG_HPRT, hprt | HPRT_PRTRST);

                            micro_delay(ms_to_micro(63));

                            write_volatile(DOTG_HPRT, hprt);
                            micro_delay(ms_to_micro(63));

                            init_fifo();
                            // //FeatureReset
                            // let mut pwr = read_volatile(DOTG_PCGCCTL);
                            // pwr &= !(1 << 5);
                            // pwr &= !(1 << 0);
                            // write_volatile(DOTG_PCGCCTL, pwr);
                            // write_volatile(DOTG_PCGCCTL, 0);

                            // let mut hprt = read_volatile(DOTG_HPRT);
                            // hprt &= !HPRT_PRTSUSP;
                            // hprt |= HPRT_PRTRST;
                            // hprt |= HPRT_PRTPWR;
                            // write_volatile(DOTG_HPRT, hprt & (0x1f140 | 0x1180));
                            // micro_delay(60000);

                            // hprt &= !HPRT_PRTRST;
                            // write_volatile(DOTG_HPRT, hprt & (0x1f140 | 0x1000));
                        }
                        8 => {
                            //FeaturePower
                            let mut hprt = read_volatile(DOTG_HPRT);
                            hprt |= HPRT_PRTPWR;
                            write_volatile(DOTG_HPRT, hprt & (0x1f140 | 0x1000));
                        }
                        _ => {}
                    }
                }
                _ => {
                    result = ResultCode::ErrorArgument;
                }
            }
        }
        UsbDeviceRequestRequest::SetAddress => {
            reply_length = 0;
            let address = request.value as u32;
            let bus = unsafe { &mut (*device.bus) };
            bus.roothub_device_number = address;
        }
        UsbDeviceRequestRequest::GetDescriptor => {
            match request.request_type {
                0x80 => {
                    match (request.value >> 8) & 0xff {
                        1 => {
                            //Device
                            reply_length =
                                min(size_of::<UsbDeviceDescriptor>(), buffer_length as usize);
                            unsafe {
                                memory_copy(
                                    buffer,
                                    (&DeviceDescriptor as *const UsbDeviceDescriptor).cast(),
                                    reply_length,
                                );
                            }
                        }
                        2 => {
                            //Configuration
                            reply_length =
                                min(size_of::<ConfigurationDescriptor>(), buffer_length as usize);

                            unsafe {
                                memory_copy(
                                    buffer,
                                    (&CONFIGURATION_DESCRIPTOR as *const ConfigurationDescriptor)
                                        .cast(),
                                    reply_length,
                                );
                            }
                        }
                        3 => {
                            //String
                            println!("| HCD.Hub: String Descriptor Not implemented.");
                            result = ResultCode::ErrorArgument;
                        }
                        _ => {
                            result = ResultCode::ErrorArgument;
                        }
                    }
                }
                0xa0 => {
                    reply_length = min(
                        HUB_DESCRIPTOR.DescriptorLength as usize,
                        buffer_length as usize,
                    );

                    unsafe {
                        memory_copy(
                            buffer,
                            (&HUB_DESCRIPTOR as *const HubDescriptor).cast(),
                            reply_length as usize,
                        );
                    }
                }
                _ => {
                    result = ResultCode::ErrorArgument;
                }
            }
        }
        UsbDeviceRequestRequest::GetConfiguration => {
            unsafe { *(buffer as *mut u8) = 1 };
            reply_length = 1;
        }
        UsbDeviceRequestRequest::SetConfiguration => {
            reply_length = 0;
        }
        _ => {
            println!("| HCD.Hub: Unsupported request.");
            result = ResultCode::ErrorArgument;
        }
    }

    if result == ResultCode::ErrorArgument {
        device.error = UsbTransferError::Stall;
    } else {
        device.error = UsbTransferError::NoError;
    }

    device.last_transfer = reply_length as u32;

    return result;
}
