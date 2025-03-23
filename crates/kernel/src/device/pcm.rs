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

const RXTHR_SINGLE_SAMPLE: usize = 0;
const RXTHR_25_PERCENT: usize = 1;
const RXTHR_75_PERCENT: usize = 2;
const RXTHR_FULL: usize = 3;

const TXTHR_EMPTY: usize = 0;
const TXTHR_25_PERCENT: usize = 1;
const TXTHR_75_PERCENT: usize = 2;
const TXTHR_SINGLE_SAMPLE: usize = 3;

const PCM_CLOCK_MASTER: usize = 0;
const PCM_CLOCK_SLAVE: usize = 1;

const PCM_CLOCK_OUTPUT_RISING: usize = 0; // default
const PCM_CLOCK_OUTPUT_FALLING: usize = 1; // inverted

const PCM_FRAME_SYNC_MASTER: usize = 0;
const PCM_FRAME_SYNC_SLAVE: usize = 1;

const PCM_FRAME_SYNC_HIGH: usize = 0; // default
const PCM_FRAME_SYNC_LOW: usize = 1; // inverted

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
            self.set_bit(CS_A, 3, true)
        }
    }

    fn reset_reception_fifo(&mut self) {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(CS_A, 4, true)
        }
    }

    fn get_transmission_fifo_threshold(&mut self) {
        self.read_bits(CS_A, 5, 2)
    }

    fn set_transmission_fifo_threshold(&mut self, val: u32) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.write_bits(CS_A, 5, 2, val);
            true
        }
    }

    fn get_reception_fifo_threshold(&mut self) {
        self.read_bits(CS_A, 7, 2)
    }

    fn set_reception_fifo_threshold(&mut self, val: u32) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.write_bits(CS_A, 7, 2, val);
            true
        }
    }

    fn dma_dreq_is_enabled(&mut self) -> bool {
        self.check_bit(CS_A, 9)
    }

    fn toggle_dma_dreq(&mut self, on: bool) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(CS_A, 9, on)
        }
    }

    fn transmission_fifo_in_sync(&mut self) -> bool {
        self.check_bit(CS_A, 13)
    }

    fn reception_fifo_in_sync(&mut self) -> bool {
        self.check_bit(CS_A, 14)
    }

    fn transmission_fifo_errored(&mut self) -> bool {
        self.check_bit(CS_A, 15)
    }

    fn reception_fifo_errored(&mut self) -> bool {
        self.check_bit(CS_A, 16)
    }

    fn clear_transmission_fifo_error(&mut self) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(CS_A, 15, true)
        }
    }

    fn clear_reception_fifo_error(&mut self) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(CS_A, 16, true)
        }
    }

    fn transmission_fifo_needs_writing(&mut self) -> bool {
        self.check_bit(CS_A, 17)
    }

    fn reception_fifo_needs_reading(&mut self) -> bool {
        self.check_bit(CS_A, 18)
    }
    
    fn transmission_fifo_available(&mut self) -> bool {
        self.check_bit(CS_A, 19)
    }

    fn reception_fifo_available(&mut self) -> bool {
        self.check_bit(CS_A, 20)
    }

    fn transmission_fifo_empty(&mut self) -> bool {
        self.check_bit(CS_A, 21)
    }
    
    fn reception_fifo_full(&mut self) -> bool {
        self.check_bit(CS_A, 22)
    }

    fn reception_sex_enabled(&mut self) -> bool {
        self.check_bit(CS_A, 23)
    }

    fn toggle_reception_sex(&mut self) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(CS_A, 23, true)
        }
    }

    // for checking the PCM clock cycle.
    // value written here will be read back 2 cycles later
    fn check_clock_sync(&mut self) -> bool {
        self.check_bit(CS_A, 24)
    }
    fn toggle_clock_sync(&mut self, on: bool) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(CS_A, 24, on)
        }
    }

    fn fifo_read(&mut self) -> u32 {
        self.read_bits(FIFO_A, 0, 32)
    }

    fn fifo_write(&mut self, data: u32) {
        self.write_bits(FIFO_A, 0, 32, data)
    }

    fn pcm_clock_disabled(&mut self) -> bool {
        self.check_bit(MODE_A, 28)
    }

    fn toggle_pcm_clock(&mut self, disabled: bool) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(MODE_A, 28, disabled)
        }
    }

    fn get_pdm_decimation_factor(&mut self) -> u32 {
        if self.check_bit(MODE_A, 27) {32} else {16}
    }

    fn set_pdm_decimation_factor(&mut self, factor: u32) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            if factor == 16 {
                self.set_bit(MODE_A, 27, false)
            }
            else if factor == 32 {
                self.set_bit(MODE_A, 27, true)
            }
            else {
                false
            }
        }
    }

    fn pdm_enabled(&mut self) -> bool {
        self.check_bit(MODE_A, 26)
    }

    fn toggle_pdm(&mut self, on: bool) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(MODE_A, 26, on)
        }
    }

    fn reception_frame_packing_enabled(&mut self) -> bool {
        self.check_bit(MODE_A, 25)
    }

    fn transmission_frame_packing_enabled(&mut self) -> bool {
        self.check_bit(MODE_A, 24)
    }

    fn toggle_reception_frame_packing(&mut self, on: bool) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(MODE_A, 25, on)
        }
    }

    fn toggle_transmission_frame_packing(&mut self, on: bool) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.set_bit(MODE_A, 24, on)
        }
    }

    fn get_pcm_clock_mode(&mut self) -> u32 {
        if self.check_bit(MODE_A, 23) { 1 } else { 0 }
    }

    fn set_pcm_clock_mode(&mut self, mode: u32) -> bool {
        if self.pcm_is_enabled() {
            false 
        }
        else { 
            self.set_bit(MODE_A, 23, if mode == 0 { false } else { true })
        }
    }

    fn get_pcm_clock_inversion(&mut self) -> u32 {
        if self.check_bit(MODE_A, 22) { 1 } else { 0 }
    }

    fn set_pcm_clock_inversion(&mut self, mode: u32) -> bool {
        if self.pcm_is_enabled() {
            false 
        }
        else { 
            self.set_bit(MODE_A, 22, if mode == 0 { false } else { true })
        }
    }

    fn get_pcm_frame_sync_mode(&mut self) -> u32 {
        if self.check_bit(MODE_A, 21) { 1 } else { 0 }
    }

    fn set_pcm_frame_sync_mode(&mut self, mode: u32) -> bool {
        if self.pcm_is_enabled() {
            false 
        }
        else { 
            self.set_bit(MODE_A, 21, if mode == 0 { false } else { true })
        }
    }

    fn get_pcm_frame_sync_inversion(&mut self) -> u32 {
        if self.check_bit(MODE_A, 20) { 1 } else { 0 }
    }

    fn set_pcm_frame_sync_inversion(&mut self, mode: u32) -> bool {
        if self.pcm_is_enabled() {
            false 
        }
        else { 
            self.set_bit(MODE_A, 20, if mode == 0 { false } else { true })
        }
    }

    fn get_pcm_frame_length(&mut self) -> u32 {
        self.read_bits(MODE_A, 10, 10) + 1
    }

    fn set_pcm_frame_length(&mut self, length: u32) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.write_bits(MODE_A, 10, 10, (length - 1) & ((1 << 10) - 1));
            true
        }
    }

    fn get_pcm_frame_sync_length(&mut self) -> u32 {
        self.read_bits(MODE_A, 0, 10)
    }

    // only in master frame sync mode
    fn set_pcm_frame_sync_length(&mut self, length: u32) -> bool {
        if self.pcm_is_enabled() {
            false
        }
        else {
            self.write_bits(MODE_A, 0, 10, length & ((1 << 10) - 1));
            true
        }
    }
}
