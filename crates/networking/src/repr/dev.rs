use crate::Result;

pub trait Device {
    fn send(&mut self, buffer: &[u8]) -> Result<()>;

    fn recv(&mut self, buffer: &mut [u8]) -> Result<usize>;

    fn max_transmission_unit(&self) -> usize;
}
