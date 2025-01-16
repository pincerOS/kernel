#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(test)]
mod tests;

#[cfg(feature = "std")]
pub mod linux;

pub const SECTOR_SIZE: usize = 512;

pub enum BlockDeviceError {
    Unknown,
}

pub trait BlockDevice {
    fn read_sector(
        &mut self,
        index: u64,
        buffer: &mut [u8; SECTOR_SIZE],
    ) -> Result<(), BlockDeviceError>;
    fn write_sector(
        &mut self,
        index: u64,
        buffer: &[u8; SECTOR_SIZE],
    ) -> Result<(), BlockDeviceError>;
}

pub struct Ext2<Device> {
    device: Device,
}

impl<D> Ext2<D>
where
    D: BlockDevice,
{
    pub fn new(device: D) -> Self {
        // TODO: init logic here

        Self { device }
    }

    // TODO: more methods
}
