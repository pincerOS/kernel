use crate::device::system_timer::{get_time, micro_delay};
/**
 *
 * usbd/endpoint.rs
 *  By Aaron Lo
 *   
 *   This file contains implemenation for USB endpoints
 *
 */
use crate::device::usb;

// use crate::device::system_timer::micro_delay;
use crate::device::usb::hcd::dwc::dwc_otg;
use crate::device::usb::hcd::dwc::dwc_otg::DwcActivateChannel;
use crate::device::usb::hcd::dwc::dwc_otg::{printDWCErrors, read_volatile, DWCSplitControlState, DWCSplitStateMachine, DwcEnableChannel, UpdateDwcOddFrame, DWC_CHANNEL_CALLBACK};
use crate::device::usb::hcd::dwc::dwc_otgreg::DOTG_HCINT;
use crate::device::usb::hcd::dwc::dwc_otgreg::{DOTG_HCTSIZ, DOTG_HFNUM, HCINT_FRMOVRUN, HCINT_NYET, HCINT_XACTERR, HFNUM_FRNUM_MASK};
use crate::device::usb::hcd::dwc::dwc_otgreg::DOTG_HCSPLT;
use crate::device::usb::hcd::dwc::dwc_otgreg::DOTG_HCCHAR;
use crate::device::usb::DwcDisableChannel;
// use crate::device::usb::hcd::dwc::dwc_otgreg::DOTG_HCINT;
use crate::device::usb::DwcFrameDifference;
// use crate::device::usb::hcd::dwc::dwc_otg;
use crate::device::usb::DwcActivateCsplit;
// use crate::device::usb::DwcDisableChannel;
use crate::device::usb::UsbSendInterruptMessage;
use crate::sync::{LockGuard, SpinLockInner};
use usb::dwc_hub;
use usb::hcd::dwc::dwc_otg::HcdUpdateTransferSize;
use usb::hcd::dwc::dwc_otgreg::{HCINT_ACK, HCINT_CHHLTD, HCINT_NAK, HCINT_XFERCOMPL};
use usb::types::*;
use usb::usbd::device::*;
use usb::usbd::pipe::*;
use usb::PacketId;


use crate::event::task::spawn_async_rt;
use crate::sync::time::{interval, MissedTicks};
use crate::SpinLock;
use alloc::boxed::Box;

// static mut NET_BUFFER_CUR_LEN: u32 = 0;
static mut NET_BUFFER_LEN: u32 = 0;
static mut NET_BUFFER_ACTIVE: bool = false;

pub static NEXT_FRAME_CS: SpinLock<u32> = SpinLock::new(0);

pub fn finish_bulk_endpoint_callback_in(endpoint: endpoint_descriptor, hcint: u32, channel: u8, _split_control: DWCSplitControlState) -> bool {
    let device = unsafe { &mut *endpoint.device };
    let mut last_transfer;
    let transfer_size = HcdUpdateTransferSize(device, channel);
    if transfer_size > endpoint.buffer_length {
        println!(
            "| Endpoint {}: transfer size {} is greater than buffer length {} in bulk in",
            channel, transfer_size, endpoint.buffer_length
        );
        let hctsiz = dwc_otg::read_volatile(DOTG_HCTSIZ(channel as usize));
        println!(
            "| Endpoint {}: hctsiz {:x} hcint {:x}",
            channel, hctsiz, hcint
        );
        last_transfer = transfer_size;
    } else {
        last_transfer = endpoint.buffer_length - transfer_size;
    }


    // let last_transfer = endpoint.buffer_length - transfer_size;
    let endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();

    if hcint & HCINT_CHHLTD == 0 {
        panic!(
            "| Endpoint {} in: HCINT_CHHLTD not set, not aborting. hcint: {:x}.",
            channel, hcint
        );
    } else if hcint & HCINT_XFERCOMPL == 0 {
        panic!(
            "| Endpoint {} in: HCINT_XFERCOMPL not set, aborting. {:x}",
            channel, hcint
        );
    }

    // println!("| Endpoint BULK RECEIVED {}: hcint {:x} len {}", channel, hcint, last_transfer);

    let dwc_sc = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };
    let dma_addr = dwc_sc.dma_addr[channel as usize];


    // let buffer = endpoint.buffer;
    // let buffer_length = device.last_transfer;
    // unsafe {
    //     core::ptr::copy_nonoverlapping(dma_addr as *const u8, buffer, buffer_length as usize);
    // }

    //assume rndis net bulk in
    unsafe {
        if !NET_BUFFER_ACTIVE {
            use alloc::slice;
            // let slice: &[u8] = unsafe { slice::from_raw_parts(dma_addr as *const u8, 16 as usize) };
            let slice32: &[u32] = slice::from_raw_parts(dma_addr as *const u32, 4 as usize);
            //print slice
            // println!("| Net buffer: {:?}", slice);
            // println!("| Net buffer 32: {:?}", slice32);
            let _buffer32 = dma_addr as *const u32;

            let rndis_len = slice32[3];
            // let part1 = unsafe { buffer32.offset(0) } as u32;
            // println!("| rndis 1 {}", part1);
            // println!(
            //     "| Net buffer length: {} rndis_len: {}",
            //     last_transfer, rndis_len
            // );
            if rndis_len > last_transfer - 44 {
                NET_BUFFER_ACTIVE = true;
                NET_BUFFER_LEN = rndis_len;
                //reenable channel
                DwcActivateChannel(channel);
                return false;
            }
            // println!("| NEt continue");
        } else {
            if last_transfer >= NET_BUFFER_LEN {
                // println!("| NEt buffer finished length: {} NETBUFFER {}", last_transfer, NET_BUFFER_LEN);
                NET_BUFFER_ACTIVE = false;
                last_transfer = NET_BUFFER_LEN;
            } else {
                // println!("| Net buffer not yet active length: {} NETBUFFER {}", last_transfer, NET_BUFFER_LEN);
                DwcActivateChannel(channel);
                return false;
            }
        }
    }

    //TODO: Perhaps update this to pass the direct dma buffer address instead of copying
    //      as it is likely that the callback will need to copy the data anyway
    //      Also, we suffer issue from buffer_length not being known before the copy so the callback likely will have better information about the buffer
    if let Some(callback) = endpoint_device.endpoints[endpoint.device_endpoint_number as usize] {
        // TODO: make this take a slice
        unsafe { callback(dma_addr as *mut u8, last_transfer) };
    } else {
        panic!(
            "| USB: No callback for endpoint number {}.",
            endpoint.device_endpoint_number
        );
    }
    return true;
}

pub fn finish_bulk_endpoint_callback_out(endpoint: endpoint_descriptor, hcint: u32, channel: u8, _split_control: DWCSplitControlState) -> bool {
    let device = unsafe { &mut *endpoint.device };
    let transfer_size = HcdUpdateTransferSize(device, channel);
    if transfer_size > endpoint.buffer_length {
        println!(
            "| Endpoint {}: transfer size {} is greater than buffer length {} in bulk out",
            channel, transfer_size, endpoint.buffer_length
        );
    }
    let last_transfer = endpoint.buffer_length - transfer_size; 

    println!(
        "Bulk out transfer hcint {:x} , last transfer: {} ",
        hcint, last_transfer
    );
    if hcint & HCINT_CHHLTD == 0 {
        panic!(
            "| Endpoint {}: HCINT_CHHLTD not set, aborting. bulk out hcint {:x}",
            channel, hcint
        );
    }

    if hcint & HCINT_XFERCOMPL == 0 {
        panic!(
            "| Endpoint {}: HCINT_XFERCOMPL not set, aborting. bulk out hcint {:x}",
            channel, hcint
        );
    }

    //Most Likely not going to be called but could be useful for cases where precise timing of when message gets off the system is needed
    let endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();
    if let Some(callback) = endpoint_device.endpoints[endpoint.device_endpoint_number as usize] {
        let mut buffer = [0]; //fake buffer
        unsafe { callback(buffer.as_mut_ptr(), last_transfer) };
    } else {
        panic!(
            "| USB: No callback for endpoint number {}.",
            endpoint.device_endpoint_number
        );
    }
    // device.last_transfer = last_transfer;
    return true;
}

pub fn finish_interrupt_endpoint_callback(endpoint: endpoint_descriptor, hcint_: u32, channel: u8, split_control: DWCSplitControlState) -> bool {
    let hcint = hcint_;
    let device = unsafe { &mut *endpoint.device };
    let transfer_size = HcdUpdateTransferSize(device, channel);
    // device.last_transfer = endpoint.buffer_length - transfer_size;
    let last_transfer = endpoint.buffer_length - transfer_size;
    let endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();

    //TODO: Hardcoded for usb-kbd for now
    let dwc_sc = unsafe { &mut *(device.soft_sc as *mut dwc_hub) };

    let dma_addr = dwc_sc.dma_addr[channel as usize];

    if hcint & HCINT_CHHLTD == 0 {
        let hcchar = dwc_otg::read_volatile(DOTG_HCCHAR(channel as usize));
        panic!(
            "| Endpoint {}: HCINT_CHHLTD not set, aborting. hcint: {:x} hcchar: {:x} finish_interrupt_endpoint_callback",
            channel, hcint, hcchar
        );
        let mut i = 0;
        let mut hcint_nochhltd = 0;
        while i < 50 {
            let hcint_nochhltd = dwc_otg::read_volatile(DOTG_HCINT(channel as usize));
            if hcint_nochhltd & HCINT_CHHLTD != 0 {
                break;
            }
            i += 1;
            micro_delay(10);
        }

        if hcint_nochhltd & HCINT_CHHLTD == 0 {
            // println!(
            //     "| Endpoint {}: HCINT_CHHLTD not set, aborting. hcint: {:x} hcint2: {:x}",
            //     channel, hcint, hcint_nochhltd
            // );
            DwcDisableChannel(channel);
            hcint_nochhltd = dwc_otg::read_volatile(DOTG_HCINT(channel as usize));
            // return true;
        }

        hcint |= hcint_nochhltd;

        return true;
    }

    let split_control_state = split_control.state;
    let ss_hfnum = split_control.ss_hfnum;

    if split_control_state == DWCSplitStateMachine::SSPLIT {
        if hcint & HCINT_NAK != 0 {
            println!("| Endpoint SSPLIT {}: NAK received hcint {:x}", channel, hcint);
            DwcEnableChannel(channel);
            return false;
        } else if hcint & HCINT_FRMOVRUN != 0 {
            println!("| Endpoint SSPLIT {}: Frame overrun hcint {:x}", channel, hcint);
            UpdateDwcOddFrame(channel);
            return false;
        } else if hcint & HCINT_XACTERR != 0 {
            println!("| Endpoint SSPLIT {}: XACTERR received hcint {:x}", channel, hcint);
            DwcEnableChannel(channel);
            return false;
        } else if hcint & HCINT_ACK != 0 {
            //ACK received
            unsafe {
                DWC_CHANNEL_CALLBACK.split_control_state[channel as usize].state = DWCSplitStateMachine::CSPLIT;
            }
            let mut cur_frame = dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK;
            let mut succeed = false;
            
            while !succeed {
                while DwcFrameDifference(cur_frame, ss_hfnum) < 2 {
                    cur_frame = dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK;
                    micro_delay(10);
                }

                unsafe {
                    let mut frame_val = NEXT_FRAME_CS.lock();
                    let current_current_frame = dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK;
                    if *frame_val == current_current_frame {
                        //not succeed
                    } else {
                        succeed = true;
                        *frame_val = current_current_frame;
                    }
                }
                micro_delay(10);
            }
            let frame = DwcActivateCsplit(channel);
            unsafe {
                DWC_CHANNEL_CALLBACK.split_control_state[channel as usize].mr_cs_hfnum = frame;
                DWC_CHANNEL_CALLBACK.split_control_state[channel as usize].tries = 1;
            }
            return false;
        } else {
            // println!("| Endpoint {}: UNKNOWWN HCINT split_control is SSPLIT hcint {:x}", channel, hcint);
            return true;
        }
    } else if split_control_state == DWCSplitStateMachine::CSPLIT {
        if hcint & HCINT_NAK != 0 {
            // println!("| Endpoint CSPLIT {}: NAK received hcint {:x}", channel, hcint);
        } else if hcint & HCINT_FRMOVRUN != 0 {
            println!("| Endpoint CSPLIT {}: Frame overrun hcint {:x}", channel, hcint);
            UpdateDwcOddFrame(channel);
            return false;
        } else if hcint & HCINT_XACTERR != 0 {
            println!("| Endpoint CSPLIT {}: XACTERR received hcint {:x}", channel, hcint);
            DwcEnableChannel(channel);
            return false;
        } else if hcint & HCINT_NYET != 0 {
            let mut cur_frame = dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK;

            if DwcFrameDifference(cur_frame, ss_hfnum) >= 8 {
                // println!("| Endpoint CSPLIT {} has exceeded 8 frames, cur_frame: {} ss_hfnum: {} giving up tries {}", channel, cur_frame, ss_hfnum, unsafe { DWC_CHANNEL_CALLBACK.split_control_state[channel as usize].tries });
                return true;
            }

            if unsafe { DWC_CHANNEL_CALLBACK.split_control_state[channel as usize].tries >= 3} {
                let hctsiz = dwc_otg::read_volatile(DOTG_HCTSIZ(channel as usize));
                // println!("| Endpoint CSPLIT {} has exceeded 3 tries, giving up hctsiz {:x} last transfer {:x} state {:?}", channel, hctsiz, last_transfer, unsafe { DWC_CHANNEL_CALLBACK.split_control_state[channel as usize] });
                return true;
            }

            let mr_cs_hfnum = unsafe {
                DWC_CHANNEL_CALLBACK.split_control_state[channel as usize].mr_cs_hfnum
            };

            let mut succeed = false;
            
            while !succeed {
                while cur_frame == mr_cs_hfnum {
                    cur_frame = dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK;
                    micro_delay(10);
                }

                unsafe {
                    let mut frame_val = NEXT_FRAME_CS.lock();
                    let current_current_frame = dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK;
                    if *frame_val == current_current_frame {
                        //not succeed
                    } else {
                        succeed = true;
                        *frame_val = current_current_frame;
                    }
                }
                micro_delay(10);
            }

            if DwcFrameDifference(cur_frame, ss_hfnum) >= 8 {
                println!("| Endpoint CSPLIT {} has exceeded 8 frames (2), cur_frame: {} ss_hfnum: {} giving up tries {}", channel, cur_frame, ss_hfnum, unsafe { DWC_CHANNEL_CALLBACK.split_control_state[channel as usize].tries });
                return true;
            }
            
            let frame = DwcEnableChannel(channel);
            unsafe {
                DWC_CHANNEL_CALLBACK.split_control_state[channel as usize].mr_cs_hfnum = frame;
                DWC_CHANNEL_CALLBACK.split_control_state[channel as usize].tries += 1;
            }
            return false;
        } else {
            unsafe {
                DWC_CHANNEL_CALLBACK.split_control_state[channel as usize].state = DWCSplitStateMachine::NONE;
            }

            if hcint & HCINT_ACK == 0 {
                let hctsiz = dwc_otg::read_volatile(DOTG_HCTSIZ(channel as usize));
                println!("| Endpoint CSPLIT {}: hcint {:x} last transfer {:x} hctisiz {:x}", channel, hcint, last_transfer, hctsiz);
    
                // use crate::device::usb::hcd::dwc::dwc_otgreg::DOTG_GINTSTS;
                // let gintsts = read_volatile(DOTG_GINTSTS);
                // use crate::device::usb::hcd::dwc::dwc_otgreg::DOTG_HCINT;
                // let hcint = read_volatile(DOTG_HCINT(channel as usize));
                // use crate::device::usb::hcd::dwc::dwc_otgreg::DOTG_HCCHAR;
                // let hcchar = read_volatile(DOTG_HCCHAR(channel as usize));
                // let hctsiz = read_volatile(DOTG_HCTSIZ(channel as usize));
    
                // println!("| HCD gintsts: {:#x}", gintsts);
                // println!("| HCD hcint: {:#x}", hcint);
                // println!("| HCD hcchar: {:#x}", hcchar);
                // println!("| HCD hctsiz: {:#x}", hctsiz);
                // println!("| HCD channel: {:#x}\n", channel);
            }
        }
    }

    let mut buffer_length = last_transfer.clamp(0, 8);
    
    if hcint & HCINT_ACK != 0 {
        endpoint_device.endpoint_pid[endpoint.device_endpoint_number as usize] += 1;
        
        if last_transfer == 0 {
            // if endpoint.buffer_length == 0 && transfer_size == 0 {
            buffer_length = 8;
            println!("| Endpoint {}: ACK received, but endpoint buffer is 0, weird. buffer len {} transfer siz {}", channel, endpoint.buffer_length, transfer_size);
            
            // }
        }
    }
        
    let mut buffer = Box::new_uninit_slice(buffer_length as usize);
    if hcint & HCINT_NAK != 0 {
        //NAK received, do nothing
        // assert_eq!(buffer_length, 0);
    } else if hcint & HCINT_XFERCOMPL != 0 {
        //Transfer complete
        //copy from dma_addr to buffer
        unsafe {
            core::ptr::copy_nonoverlapping(
                dma_addr as *const u8,
                buffer.as_mut_ptr().cast(),
                buffer_length as usize,
            );
        }
    } else if hcint & HCINT_FRMOVRUN != 0 {
        //Frame overrun
        UpdateDwcOddFrame(channel);

        return false;
    } else {
        println!("| Endpoint {}: Unknown interrupt, ignoring {:x} state {:#?}. Letting run for now...", channel, hcint, split_control);
        // return true;
    }

    let mut buffer = unsafe { buffer.assume_init() };

    if let Some(callback) = endpoint_device.endpoints[endpoint.device_endpoint_number as usize] {
        unsafe { callback(buffer.as_mut_ptr(), buffer_length) };
    } else {
        panic!(
            "| USB: No callback for endpoint number {}.",
            endpoint.device_endpoint_number
        );
    }

    // device.last_transfer = last_transfer;
    return true;
}

pub fn interrupt_endpoint_callback(endpoint: endpoint_descriptor) {
    let device = unsafe { &mut *endpoint.device };
    let pipe = UsbPipeAddress {
        transfer_type: UsbTransfer::Interrupt,
        speed: device.speed,
        end_point: endpoint.endpoint_address,
        device: device.number as u8,
        direction: endpoint.endpoint_direction,
        max_size: endpoint.max_packet_size,
        _reserved: 0,
    };

    let endpoint_device = device.driver_data.downcast::<UsbEndpointDevice>().unwrap();
    let pid = if endpoint_device.endpoint_pid[endpoint.device_endpoint_number as usize] % 2 == 0 {
        PacketId::Data0
    } else {
        PacketId::Data1
    };
    
    let result = unsafe {
        UsbSendInterruptMessage(
            device,
            pipe,
            8,
            pid,
            endpoint.timeout,
            finish_interrupt_endpoint_callback,
            endpoint,
        )
    };

    if result != ResultCode::OK {
        print!("| USB: Failed to read interrupt endpoint.\n");
    }
}

pub fn register_interrupt_endpoint(
    device: &mut UsbDevice,
    endpoint_time: u32,
    endpoint_address: u8,
    endpoint_direction: UsbDirection,
    endpoint_max_size: UsbPacketSize,
    device_endpoint_number: u8,
    timeout: u32,
) {
    let endpoint = endpoint_descriptor {
        endpoint_address: endpoint_address as u8,
        endpoint_direction: endpoint_direction,
        endpoint_type: UsbTransfer::Interrupt,
        max_packet_size: endpoint_max_size,
        device_endpoint_number: device_endpoint_number,
        device: device,
        device_number: device.number,
        device_speed: device.speed,
        buffer_length: 8,
        // buffer: core::ptr::null_mut(),
        timeout: timeout,
    };

    spawn_async_rt(async move {
        let μs = endpoint_time as u64 * 1000;
        let mut interval = interval(μs).with_missed_tick_behavior(MissedTicks::Skip);
        println!("| USB: Starting interrupt endpoint with interval {} μs", μs);

        let hf1 = unsafe { dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK };
        let hf1_time = get_time();
        let mut hf2 = unsafe { dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK };
        let mut hf2_time = get_time();
        while hf1 == hf2 {
            hf2 = unsafe { dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK };
            hf2_time = get_time();
        }
        let mut hf3 = unsafe { dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK };
        let mut hf3_time = get_time();
        while hf2 == hf3 {
            hf3 = unsafe { dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK };
            hf3_time = get_time();
        }
        let mut hf4 = unsafe { dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK };
        let mut hf4_time = get_time();
        while hf3 == hf4 {
            hf4 = unsafe { dwc_otg::read_volatile(DOTG_HFNUM) & HFNUM_FRNUM_MASK };
            hf4_time = get_time();
        }
        println!("| USB: HFNUM: {} {} {} {} {} {} {} {}", hf1, hf1_time, hf2, hf2_time, hf3, hf3_time, hf4, hf4_time);

        let mut prev_time = get_time();
        while interval.tick().await {
            let cur_time = get_time();
            if cur_time - prev_time < μs {

            } else {
                interrupt_endpoint_callback(endpoint);
            }
            prev_time = cur_time;
        }
    });
}

#[derive(Copy, Clone)]
pub struct endpoint_descriptor {
    pub endpoint_address: u8,
    pub endpoint_direction: UsbDirection,
    pub endpoint_type: UsbTransfer,
    pub max_packet_size: UsbPacketSize,
    pub device_endpoint_number: u8,
    pub device: *mut UsbDevice,
    pub device_number: u32,
    pub device_speed: UsbSpeed,
    pub buffer_length: u32,
    // pub buffer: *mut u8,
    pub timeout: u32,
}

unsafe impl Sync for endpoint_descriptor {}
unsafe impl Send for endpoint_descriptor {}

impl endpoint_descriptor {
    pub fn new() -> Self {
        endpoint_descriptor {
            endpoint_address: 0,
            endpoint_direction: UsbDirection::Out,
            endpoint_type: UsbTransfer::Control,
            max_packet_size: UsbPacketSize::Bits8,
            device_endpoint_number: 0,
            device: core::ptr::null_mut(),
            device_number: 0,
            device_speed: UsbSpeed::Low,
            buffer_length: 0,
            // buffer: core::ptr::null_mut(),
            timeout: 0,
        }
    }
}

impl UsbEndpointDevice {
    pub fn new() -> Self {
        UsbEndpointDevice {
            endpoints: [None; 5],
            endpoint_pid: [0; 5],
        }
    }
}

pub struct UsbEndpointDevice {
    //TODO: update for better?: The 5 is an arbitrary number
    pub endpoints: [Option<unsafe fn(*mut u8, u32)>; 5],
    pub endpoint_pid: [usize; 5],
}
