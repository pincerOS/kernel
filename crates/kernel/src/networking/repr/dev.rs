pub trait Device {
    // fn send(&mut self, buffer: &[u8]) -> Result<()>;
    fn send(&mut self, buffer: &mut [u8], buffer_len: u32);

    // fn recv(&mut self, buffer: &mut [u8], buffer_len: u32) -> Result<usize>;
    fn recv(&mut self, buffer: &mut [u8], buffer_len: u32);

    fn mtu(&self) -> usize;
}
