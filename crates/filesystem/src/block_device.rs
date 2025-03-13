use crate::Ext2Error;

pub const SECTOR_SIZE: usize = 512;
#[derive(Debug, PartialEq)]
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

    fn read_sectors(
        &mut self,
        start_index: u64,
        buffer: &mut [u8],
    ) -> Result<(), BlockDeviceError> {
        assert!(buffer.len() % SECTOR_SIZE == 0);
        for (buf_segment, sector) in buffer.chunks_exact_mut(SECTOR_SIZE).zip(start_index..) {
            let array: &mut [u8; SECTOR_SIZE] = buf_segment.try_into().unwrap();
            self.read_sector(sector, array)?;
        }
        Ok(())
    }

    fn write_sectors(
        &mut self,
        start_index: u64,
        sectors: usize,
        buffer: &[u8],
    ) -> Result<(), BlockDeviceError> {
        let mut tmp_buf: [u8; SECTOR_SIZE] = [0; SECTOR_SIZE];
        for i in 0..sectors {
            let cur_sector = start_index + (i as u64);
            for j in 0..SECTOR_SIZE {
                tmp_buf[j] = buffer[(i * SECTOR_SIZE) + j];
            }
            self.write_sector(cur_sector, &tmp_buf)?;
        }
        Ok(())
    }
}
