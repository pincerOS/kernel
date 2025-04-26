use crate::device::usb::hcd::dwc::dwc_otg::PacketId;
use crate::device::usb::types::UsbTransfer;
/**
 *
 * usbd/transfer.rs
 *  By Aaron Lo
 *   
 *   This file contains implemenation for USB transfers
 *
 */
use crate::device::usb::usbd::pipe::UsbPipeAddress;
use crate::ringbuffer::SpscRingBuffer;
use crate::sync::InterruptSpinLock;
use alloc::boxed::Box;

use super::endpoint::*;
pub const RING_BUFFER_SIZE: usize = 4096;
pub struct UsbTransferQueue {
    pub interrupt_queue: InterruptSpinLock<SpscRingBuffer<RING_BUFFER_SIZE, Box<UsbXfer>>>,
    pub bulk_queue: InterruptSpinLock<SpscRingBuffer<RING_BUFFER_SIZE, Box<UsbXfer>>>,
}

unsafe impl Sync for UsbTransferQueue {}

impl UsbTransferQueue {
    pub const fn new() -> Self {
        UsbTransferQueue {
            interrupt_queue: InterruptSpinLock::new(SpscRingBuffer::new()),
            bulk_queue: InterruptSpinLock::new(SpscRingBuffer::new()),
        }
    }

    pub fn add_transfer(
        &self,
        transfer: Box<UsbXfer>,
        transfer_type: UsbTransfer,
    ) -> Result<(), Box<UsbXfer>> {
        let transfer_result;
        match transfer_type {
            UsbTransfer::Interrupt => unsafe {
                transfer_result = self.interrupt_queue.lock().try_send(transfer);
            },
            UsbTransfer::Bulk => unsafe {
                transfer_result = self.bulk_queue.lock().try_send(transfer);
            },
            _ => {
                panic!("Unsupported transfer type");
            }
        }

        transfer_result
    }

    pub fn get_transfer(&self) -> Option<Box<UsbXfer>> {
        let mut transfer_result;
        unsafe {
            transfer_result = self.interrupt_queue.lock().try_recv();
        }
        if transfer_result.is_none() {
            unsafe {
                transfer_result = self.bulk_queue.lock().try_recv();
            }
        }
        transfer_result
    }
}

#[derive(Clone)]
pub struct UsbXfer {
    pub endpoint_descriptor: endpoint_descriptor,
    pub buffer: Option<Box<[u8]>>,
    pub buffer_length: u32,
    pub callback: Option<fn(endpoint_descriptor, u32, u8) -> bool>,
    pub packet_id: PacketId,
    pub pipe: UsbPipeAddress,
}
