use crate::networking::repr::dev::Device;

use crate::device::usb::device::net::{NetReceive, NetSendPacket};

pub struct CDCECM {
    max_transmission_unit: usize,
}

impl CDCECM {
    pub fn new(mtu: usize) -> Self {
        CDCECM {
            max_transmission_unit: mtu,
        }
    }

    pub fn send(&mut self, buffer: &mut [u8], buffer_len: u32) {
        unsafe {
            println!("| NET: Send packet of size {}", buffer_len);
            NetSendPacket(buffer.as_mut_ptr(), buffer_len);
        }
    }

    // TODO: fn recv(&mut self, buffer: &mut [u8], buffer_len: u32) -> Result<usize> {
    pub fn recv(&mut self, buffer: &mut [u8], buffer_len: u32) {
        unsafe {
            NetReceive(buffer.as_mut_ptr(), buffer_len);
        }
    }

    pub fn mtu(&self) -> usize {
        self.max_transmission_unit
    }
}

impl Device for CDCECM {
    // TODO: fn send(&mut self, buffer: &[u8], buffer_len: u32) -> Result<()> {
    fn send(&mut self, buffer: &mut [u8], buffer_len: u32) {
        unsafe {
            println!("| NET: Send packet of size {}", buffer_len);
            NetSendPacket(buffer.as_mut_ptr(), buffer_len);
        }
    }

    // TODO: fn recv(&mut self, buffer: &mut [u8], buffer_len: u32) -> Result<usize> {
    fn recv(&mut self, buffer: &mut [u8], buffer_len: u32) {
        unsafe {
            NetReceive(buffer.as_mut_ptr(), buffer_len);
        }
    }

    fn mtu(&self) -> usize {
        self.max_transmission_unit
    }
}

// impl Drop for CDCECM {
//     fn drop(&mut self) {
//
//     }
// }
