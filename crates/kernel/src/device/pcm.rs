// https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf

use crate::sync::Volatile;

// PCM Register Offsets
const CS_A: usize = 0x00; // PCM Control and Status
const FIFO_A: usize = 0x04; // PCM FIFO Data
const MODE_A: usize = 0x08; // PCM Mode
const RXC_A: usize = 0x0c; // PCM Receive Configuration
const TXC_A: usize = 0x10; // PCM Transmit Configuration
const DREQ_A: usize = 0x14; // PCM DMA Request Level
const INTEN_A: usize = 0x18; // PCM Interrupt Enables
const INTSTC_A: usize = 0x1c; // PCM Interrupt Status & Clear
const GRAY: usize = 0x20; // PCM Gray Mode Control

pub struct bcm2711_pcm_driver {
    base_addr: *mut (), /// 0x7e203000
}

impl bcm2711_pcm_driver {
    pub unsafe fn init(base_addr: *mut ()) -> Self {
        Self { base_addr }
    }

    fn reg_cs(&self) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(CS_A)
                .cast::<u32>(),
        )
    }

    fn reg_fifo(&self) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(FIFO_A)
                .cast::<u32>(),
        )
    }

    fn reg_mode(&self) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(MODE_A)
                .cast::<u32>(),
        )
    }

    fn reg_rxc(&self) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(RXC_A)
                .cast::<u32>(),
        )
    }

    fn reg_txc(&self) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(TXC_A)
                .cast::<u32>(),
        )
    }

    fn reg_dreq(&self) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(DREQ_A)
                .cast::<u32>(),
        )
    }

    fn reg_inten(&self) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(INTEN_A)
                .cast::<u32>(),
        )
    }

    fn reg_intstc(&self) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(INTSTC_A)
                .cast::<u32>(),
        )
    }

    fn reg_gray(&self) -> Volatile<u32> {
        Volatile(
            self.base_addr
                .wrapping_byte_add(GRAY)
                .cast::<u32>(),
        )
    }

    fn check_bit(&mut self, reg: usize, bit: usize) -> bool {
        unsafe { self.reg_cs.read() & (1 << bit) != 0 }
    }
    
    fn read_bits(&mut self, reg: usize, start: usize, len: usize) -> u32 {
        unsafe { (self.reg_cs.read() & (((1 << len) - 1) << start)) >> start }
    }

    fn set_bit(&mut self, reg: usize, bit: usize, on: bool) -> bool {
        if self.check_bit(reg, bit) == on {
            false
        } else {
            self.set_bit_force(reg, bit, on);
            true
        }
    }

    fn set_bit_force(&mut self, reg: usize, bit: usize, on: bool) {
        unsafe { self.reg_cs.write(if on {self.reg_cs.read() | (1 << bit)} else {self.reg_cs.read() & !(1 << bit)}) };
    }

    fn write_bits(&mut self, reg: usize, start: usize, len: usize, bits: u32) {
        unsafe { self.reg_cs.write(self.reg_cs.read() | ((bits & ((1 << len) - 1)) << start)) }
    }

    fn pcm_is_enabled(&mut self) -> bool {
        self.check_bit(CS_A, 0)
    }

    fn toggle_pcm(&mut self, on: bool) -> bool {
        self.set_bit(CS_A, 0, on)
    }

    fn reception_is_enabled(&mut self) -> bool {
        self.check_bit(CS_A, 1)
    }

    fn toggle_reception(&mut self, on: bool) -> bool {
        self.set_bit(CS_A, 1, on)
    }

    fn transmission_is_enabled(&mut self) -> bool {
        self.check_bit(CS_A, 2)
    }

    fn toggle_transmission(&mut self, on: bool) -> bool {
        self.set_bit(CS_A, 2, on)
    }

    fn reset_transmission_fifo(&mut self) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(CS_A, 3, true);
            true
        }
    }

    fn reset_reception_fifo(&mut self) {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(CS_A, 4, true);
            true
        }
    }

    fn get_transmission_fifo_threshold(&self) {
        self.read_bits(CS_A, 5, 2)
    }

    fn set_transmission_fifo_threshold(&self, val: u32) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.write_bits(CS_A, 5, 2, val);
            true
        }
    }

    fn get_reception_fifo_threshold(&self) {
        self.read_bits(CS_A, 7, 2)
    }

    fn set_reception_fifo_threshold(&self, val: u32) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.write_bits(CS_A, 7, 2, val);
            true
        }
    }
}
