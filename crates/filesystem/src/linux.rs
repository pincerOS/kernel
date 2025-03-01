#[cfg(feature = "std")]
extern crate std;

use crate::{BlockDevice, BlockDeviceError, SECTOR_SIZE};

pub struct FileBlockDevice {
    file: std::fs::File,
}

impl FileBlockDevice {
    pub fn new(file: std::fs::File) -> Self {
        Self { file }
    }
}

impl BlockDevice for FileBlockDevice {
    fn read_sector(
        &mut self,
        index: u64,
        buffer: &mut [u8; SECTOR_SIZE],
    ) -> Result<(), BlockDeviceError> {
        use std::os::unix::fs::FileExt;

        self.file
            .read_exact_at(buffer, index * SECTOR_SIZE as u64)
            .map_err(|_| BlockDeviceError::Unknown)?;

        Ok(())
    }

    fn write_sector(
        &mut self,
        index: u64,
        buffer: &[u8; SECTOR_SIZE],
    ) -> Result<(), BlockDeviceError> {
        use std::os::unix::fs::FileExt;

        self.file
            .write_all_at(buffer, index * SECTOR_SIZE as u64)
            .map_err(|_| BlockDeviceError::Unknown)?;

        Ok(())
    }
}
