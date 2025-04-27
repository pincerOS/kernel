#![allow(dead_code, nonstandard_style)]

pub static SD: UnsafeInit<SpinLock<bcm2711_emmc2_driver>> = unsafe { UnsafeInit::uninit() };

// SDHCI driver for raspberry pi 4b
// TODO:
// spin_sleep() timing validation
// cleanup error handling
// cleanup comments

// https://github.com/jncronin/rpi-boot/blob/master/emmc.c
// https://github.com/rsta2/circle/blob/master/addon/SDCard/emmc.cpp

use alloc::boxed::Box;

use crate::arch::get_time_ticks;
use crate::device::mailbox::PropSetPowerState;
use crate::sync::{spin_sleep, SpinLock, UnsafeInit, Volatile};
use filesystem::BlockDevice;

// use super::{gpio, GPIO};
use super::{mailbox, MAILBOX};

#[derive(Debug, Copy, Clone)]
pub struct SdScr {
    pub scr: [u32; 2],
    pub sd_bus_widths: u32,
    pub sd_version: u32,
}

#[derive(Debug)]
pub struct EmmcBlockDev {
    pub ullOffset: u64,

    pub hci_ver: u32,

    pub id: [u32; 4],

    pub csd: [u32; 4],
    pub capacity: u64,

    pub card_supports_sdhc: u32,
    pub card_supports_hs: u32,
    pub card_supports_18v: u32,
    pub card_ocr: u32,
    pub card_rca: u32,
    pub last_interrupt: u32,
    pub last_error: u32,

    pub scr: *mut SdScr,

    pub failed_voltage_switch: i32,

    pub last_cmd_reg: u32,
    pub last_cmd: u32,
    pub last_cmd_success: u32,
    pub last_r0: u32,
    pub last_r1: u32,
    pub last_r2: u32,
    pub last_r3: u32,

    pub buf: *mut (),
    pub blocks_to_transfer: i32,
    pub block_size: u32,
    pub use_sdma: i32,
    pub card_removal: i32,
    pub base_clock: u32,
}

const CARD_RCA_INVALID: u32 = 0xffff0000;

// Register offsets
const EMMC_BASE: u32 = 0xfe340000;
const ARG2: usize = 0x00;
const BLKSIZECNT: usize = 0x04;
const ARG1: usize = 0x08;
const CMDTM: usize = 0x0C;
const RESP0: usize = 0x10;
const RESP1: usize = 0x14;
const RESP2: usize = 0x18;
const RESP3: usize = 0x1C;
const DATA: usize = 0x20;
const STATUS: usize = 0x24;
const CONTROL0: usize = 0x28;
const CONTROL1: usize = 0x2C;
const INTERRUPT: usize = 0x30;
const IRPT_MASK: usize = 0x34;
const IRPT_EN: usize = 0x38;
const CONTROL2: usize = 0x3C;
const CAPABILITIES_0: usize = 0x40;
const CAPABILITIES_1: usize = 0x44;
const FORCE_IRPT: usize = 0x50;
const BOOT_TIMEOUT: usize = 0x70;
const DBG_SEL: usize = 0x74;
const EXRDFIFO_CFG: usize = 0x80;
const EXRDFIFO_EN: usize = 0x84;
const TUNE_STEP: usize = 0x88;
const TUNE_STEPS_STD: usize = 0x8C;
const TUNE_STEPS_DDR: usize = 0x90;
const SPI_INT_SPT: usize = 0xF0;
const SLOTISR_VER: usize = 0xFC;

const SD_CLOCK_ID: u32 = 400000;
const SD_CLOCK_NORMAL: u32 = 25000000;
const SD_CLOCK_HIGH: u32 = 50000000;
const SD_CLOCK_100: u32 = 100000000;
const SD_CLOCK_208: u32 = 208000000;

const SDMA_BUFFER: usize = 0x6000;
const SDMA_BUFFER_PA: usize = SDMA_BUFFER + 0xC0000000;

pub const fn sd_cmd_index(a: u32) -> u32 {
    a << 24
}

pub const SD_CMD_TYPE_NORMAL: u32 = 0x0;
pub const SD_CMD_TYPE_SUSPEND: u32 = 1 << 22;
pub const SD_CMD_TYPE_RESUME: u32 = 2 << 22;
pub const SD_CMD_TYPE_ABORT: u32 = 3 << 22;
pub const SD_CMD_TYPE_MASK: u32 = 3 << 22;

pub const SD_CMD_ISDATA: u32 = 1 << 21;
pub const SD_CMD_IXCHK_EN: u32 = 1 << 20;
pub const SD_CMD_CRCCHK_EN: u32 = 1 << 19;

pub const SD_CMD_RSPNS_TYPE_NONE: u32 = 0;
pub const SD_CMD_RSPNS_TYPE_136: u32 = 1 << 16;
pub const SD_CMD_RSPNS_TYPE_48: u32 = 2 << 16;
pub const SD_CMD_RSPNS_TYPE_48B: u32 = 3 << 16;
pub const SD_CMD_RSPNS_TYPE_MASK: u32 = 3 << 16;

pub const SD_CMD_MULTI_BLOCK: u32 = 1 << 5;
pub const SD_CMD_DAT_DIR_HC: u32 = 0;
pub const SD_CMD_DAT_DIR_CH: u32 = 1 << 4;

pub const SD_CMD_AUTO_CMD_EN_NONE: u32 = 0;
pub const SD_CMD_AUTO_CMD_EN_CMD12: u32 = 1 << 2;
pub const SD_CMD_AUTO_CMD_EN_CMD23: u32 = 2 << 2;

pub const SD_CMD_BLKCNT_EN: u32 = 1 << 1;
pub const SD_CMD_DMA: u32 = 1;

pub const SD_ERR_CMD_TIMEOUT: u32 = 0;
pub const SD_ERR_CMD_CRC: u32 = 1;
pub const SD_ERR_CMD_END_BIT: u32 = 2;
pub const SD_ERR_CMD_INDEX: u32 = 3;
pub const SD_ERR_DATA_TIMEOUT: u32 = 4;
pub const SD_ERR_DATA_CRC: u32 = 5;
pub const SD_ERR_DATA_END_BIT: u32 = 6;
pub const SD_ERR_CURRENT_LIMIT: u32 = 7;
pub const SD_ERR_AUTO_CMD12: u32 = 8;
pub const SD_ERR_ADMA: u32 = 9;
pub const SD_ERR_TUNING: u32 = 10;
pub const SD_ERR_RSVD: u32 = 11;

pub const SD_ERR_MASK_CMD_TIMEOUT: u32 = 1 << (16 + SD_ERR_CMD_TIMEOUT);
pub const SD_ERR_MASK_CMD_CRC: u32 = 1 << (16 + SD_ERR_CMD_CRC);
pub const SD_ERR_MASK_CMD_END_BIT: u32 = 1 << (16 + SD_ERR_CMD_END_BIT);
pub const SD_ERR_MASK_CMD_INDEX: u32 = 1 << (16 + SD_ERR_CMD_INDEX);
pub const SD_ERR_MASK_DATA_TIMEOUT: u32 = 1 << (16 + SD_ERR_DATA_TIMEOUT);
pub const SD_ERR_MASK_DATA_CRC: u32 = 1 << (16 + SD_ERR_DATA_CRC);
pub const SD_ERR_MASK_DATA_END_BIT: u32 = 1 << (16 + SD_ERR_DATA_END_BIT);
pub const SD_ERR_MASK_CURRENT_LIMIT: u32 = 1 << (16 + SD_ERR_CURRENT_LIMIT);
pub const SD_ERR_MASK_AUTO_CMD12: u32 = 1 << (16 + SD_ERR_AUTO_CMD12);
pub const SD_ERR_MASK_ADMA: u32 = 1 << (16 + SD_ERR_ADMA);
pub const SD_ERR_MASK_TUNING: u32 = 1 << (16 + SD_ERR_TUNING);

pub const SD_COMMAND_COMPLETE: u32 = 1;
pub const SD_TRANSFER_COMPLETE: u32 = 1 << 1;
pub const SD_BLOCK_GAP_EVENT: u32 = 1 << 2;
pub const SD_DMA_INTERRUPT: u32 = 1 << 3;
pub const SD_BUFFER_WRITE_READY: u32 = 1 << 4;
pub const SD_BUFFER_READ_READY: u32 = 1 << 5;
pub const SD_CARD_INSERTION: u32 = 1 << 6;
pub const SD_CARD_REMOVAL: u32 = 1 << 7;
pub const SD_CARD_INTERRUPT: u32 = 1 << 8;

pub const SD_RESP_NONE: u32 = SD_CMD_RSPNS_TYPE_NONE;
pub const SD_RESP_R1: u32 = SD_CMD_RSPNS_TYPE_48 | SD_CMD_CRCCHK_EN;
pub const SD_RESP_R1b: u32 = SD_CMD_RSPNS_TYPE_48B | SD_CMD_CRCCHK_EN;
pub const SD_RESP_R2: u32 = SD_CMD_RSPNS_TYPE_136 | SD_CMD_CRCCHK_EN;
pub const SD_RESP_R3: u32 = SD_CMD_RSPNS_TYPE_48;
pub const SD_RESP_R4: u32 = SD_CMD_RSPNS_TYPE_136;
pub const SD_RESP_R5: u32 = SD_CMD_RSPNS_TYPE_48 | SD_CMD_CRCCHK_EN;
pub const SD_RESP_R5b: u32 = SD_CMD_RSPNS_TYPE_48B | SD_CMD_CRCCHK_EN;
pub const SD_RESP_R6: u32 = SD_CMD_RSPNS_TYPE_48 | SD_CMD_CRCCHK_EN;
pub const SD_RESP_R7: u32 = SD_CMD_RSPNS_TYPE_48 | SD_CMD_CRCCHK_EN;

pub const SD_DATA_READ: u32 = SD_CMD_ISDATA | SD_CMD_DAT_DIR_CH;
pub const SD_DATA_WRITE: u32 = SD_CMD_ISDATA | SD_CMD_DAT_DIR_HC;

pub const fn sd_cmd_reserved(_: u32) -> u32 {
    0xffffffff
}

impl EmmcBlockDev {
    pub fn success(&self) -> bool {
        self.last_cmd_success != 0
    }

    pub fn fail(&self) -> bool {
        self.last_cmd_success == 0
    }

    pub fn timeout(&self) -> bool {
        self.fail() && (self.last_error == 0)
    }

    pub fn cmd_timeout(&self) -> bool {
        self.fail() && (self.last_error & (1 << 16)) != 0
    }

    pub fn cmd_crc(&self) -> bool {
        self.fail() && (self.last_error & (1 << 17)) != 0
    }

    pub fn cmd_end_bit(&self) -> bool {
        self.fail() && (self.last_error & (1 << 18)) != 0
    }

    pub fn cmd_index(&self) -> bool {
        self.fail() && (self.last_error & (1 << 19)) != 0
    }

    pub fn data_timeout(&self) -> bool {
        self.fail() && (self.last_error & (1 << 20)) != 0
    }

    pub fn data_crc(&self) -> bool {
        self.fail() && (self.last_error & (1 << 21)) != 0
    }

    pub fn data_end_bit(&self) -> bool {
        self.fail() && (self.last_error & (1 << 22)) != 0
    }

    pub fn current_limit(&self) -> bool {
        self.fail() && (self.last_error & (1 << 23)) != 0
    }

    pub fn acmd12_error(&self) -> bool {
        self.fail() && (self.last_error & (1 << 24)) != 0
    }

    pub fn adma_error(&self) -> bool {
        self.fail() && (self.last_error & (1 << 25)) != 0
    }

    pub fn tuning_error(&self) -> bool {
        self.fail() && (self.last_error & (1 << 26)) != 0
    }
}

pub const SD_VER_UNKNOWN: u32 = 0;
pub const SD_VER_1: u32 = 1;
pub const SD_VER_1_1: u32 = 2;
pub const SD_VER_2: u32 = 3;
pub const SD_VER_3: u32 = 4;
pub const SD_VER_4: u32 = 5;

static SD_VERSIONS: [&str; 6] = ["unknown", "1.0 and 1.01", "1.10", "2.00", "3.0x", "4.xx"];

static sd_commands: [u32; 64] = [
    sd_cmd_index(0),
    sd_cmd_reserved(1),
    sd_cmd_index(2) | SD_RESP_R2,
    sd_cmd_index(3) | SD_RESP_R6,
    sd_cmd_index(4),
    sd_cmd_index(5) | SD_RESP_R4,
    sd_cmd_index(6) | SD_RESP_R1,
    sd_cmd_index(7) | SD_RESP_R1b,
    sd_cmd_index(8) | SD_RESP_R7,
    sd_cmd_index(9) | SD_RESP_R2,
    sd_cmd_index(10) | SD_RESP_R2,
    sd_cmd_index(11) | SD_RESP_R1,
    sd_cmd_index(12) | SD_RESP_R1b | SD_CMD_TYPE_ABORT,
    sd_cmd_index(13) | SD_RESP_R1,
    sd_cmd_reserved(14),
    sd_cmd_index(15),
    sd_cmd_index(16) | SD_RESP_R1,
    sd_cmd_index(17) | SD_RESP_R1 | SD_DATA_READ,
    sd_cmd_index(18) | SD_RESP_R1 | SD_DATA_READ | SD_CMD_MULTI_BLOCK | SD_CMD_BLKCNT_EN,
    sd_cmd_index(19) | SD_RESP_R1 | SD_DATA_READ,
    sd_cmd_index(20) | SD_RESP_R1b,
    sd_cmd_reserved(21),
    sd_cmd_reserved(22),
    sd_cmd_index(23) | SD_RESP_R1,
    sd_cmd_index(24) | SD_RESP_R1 | SD_DATA_WRITE,
    sd_cmd_index(25) | SD_RESP_R1 | SD_DATA_WRITE | SD_CMD_MULTI_BLOCK | SD_CMD_BLKCNT_EN,
    sd_cmd_reserved(26),
    sd_cmd_index(27) | SD_RESP_R1 | SD_DATA_WRITE,
    sd_cmd_index(28) | SD_RESP_R1b,
    sd_cmd_index(29) | SD_RESP_R1b,
    sd_cmd_index(30) | SD_RESP_R1 | SD_DATA_READ,
    sd_cmd_reserved(31),
    sd_cmd_index(32) | SD_RESP_R1,
    sd_cmd_index(33) | SD_RESP_R1,
    sd_cmd_reserved(34),
    sd_cmd_reserved(35),
    sd_cmd_reserved(36),
    sd_cmd_reserved(37),
    sd_cmd_index(38) | SD_RESP_R1b,
    sd_cmd_reserved(39),
    sd_cmd_reserved(40),
    sd_cmd_reserved(41),
    sd_cmd_reserved(42) | SD_RESP_R1,
    sd_cmd_reserved(43),
    sd_cmd_reserved(44),
    sd_cmd_reserved(45),
    sd_cmd_reserved(46),
    sd_cmd_reserved(47),
    sd_cmd_reserved(48),
    sd_cmd_reserved(49),
    sd_cmd_reserved(50),
    sd_cmd_reserved(51),
    sd_cmd_reserved(52),
    sd_cmd_reserved(53),
    sd_cmd_reserved(54),
    sd_cmd_index(55) | SD_RESP_R1,
    sd_cmd_index(56) | SD_RESP_R1 | SD_CMD_ISDATA,
    sd_cmd_reserved(57),
    sd_cmd_reserved(58),
    sd_cmd_reserved(59),
    sd_cmd_reserved(60),
    sd_cmd_reserved(61),
    sd_cmd_reserved(62),
    sd_cmd_reserved(63),
];

static sd_acommands: [u32; 64] = [
    sd_cmd_reserved(0),
    sd_cmd_reserved(1),
    sd_cmd_reserved(2),
    sd_cmd_reserved(3),
    sd_cmd_reserved(4),
    sd_cmd_reserved(5),
    sd_cmd_index(6) | SD_RESP_R1,
    sd_cmd_reserved(7),
    sd_cmd_reserved(8),
    sd_cmd_reserved(9),
    sd_cmd_reserved(10),
    sd_cmd_reserved(11),
    sd_cmd_reserved(12),
    sd_cmd_index(13) | SD_RESP_R1,
    sd_cmd_reserved(14),
    sd_cmd_reserved(15),
    sd_cmd_reserved(16),
    sd_cmd_reserved(17),
    sd_cmd_reserved(18),
    sd_cmd_reserved(19),
    sd_cmd_reserved(20),
    sd_cmd_reserved(21),
    sd_cmd_index(22) | SD_RESP_R1 | SD_DATA_READ,
    sd_cmd_index(23) | SD_RESP_R1,
    sd_cmd_reserved(24),
    sd_cmd_reserved(25),
    sd_cmd_reserved(26),
    sd_cmd_reserved(27),
    sd_cmd_reserved(28),
    sd_cmd_reserved(29),
    sd_cmd_reserved(30),
    sd_cmd_reserved(31),
    sd_cmd_reserved(32),
    sd_cmd_reserved(33),
    sd_cmd_reserved(34),
    sd_cmd_reserved(35),
    sd_cmd_reserved(36),
    sd_cmd_reserved(37),
    sd_cmd_reserved(38),
    sd_cmd_reserved(39),
    sd_cmd_reserved(40),
    sd_cmd_index(41) | SD_RESP_R3,
    sd_cmd_index(42) | SD_RESP_R1,
    sd_cmd_reserved(43),
    sd_cmd_reserved(44),
    sd_cmd_reserved(45),
    sd_cmd_reserved(46),
    sd_cmd_reserved(47),
    sd_cmd_reserved(48),
    sd_cmd_reserved(49),
    sd_cmd_reserved(50),
    sd_cmd_index(51) | SD_RESP_R1 | SD_DATA_READ,
    sd_cmd_reserved(52),
    sd_cmd_reserved(53),
    sd_cmd_reserved(54),
    sd_cmd_reserved(55),
    sd_cmd_reserved(56),
    sd_cmd_reserved(57),
    sd_cmd_reserved(58),
    sd_cmd_reserved(59),
    sd_cmd_reserved(60),
    sd_cmd_reserved(61),
    sd_cmd_reserved(62),
    sd_cmd_reserved(63),
];

pub const GO_IDLE_STATE: u32 = 0;
pub const ALL_SEND_CID: u32 = 2;
pub const SEND_RELATIVE_ADDR: u32 = 3;
pub const SET_DSR: u32 = 4;
pub const IO_SET_OP_COND: u32 = 5;
pub const SWITCH_FUNC: u32 = 6;
pub const SELECT_CARD: u32 = 7;
pub const DESELECT_CARD: u32 = 7;
pub const SELECT_DESELECT_CARD: u32 = 7;
pub const SEND_IF_COND: u32 = 8;
pub const SEND_CSD: u32 = 9;
pub const SEND_CID: u32 = 10;
pub const VOLTAGE_SWITCH: u32 = 11;
pub const STOP_TRANSMISSION: u32 = 12;
pub const SEND_STATUS: u32 = 13;
pub const GO_INACTIVE_STATE: u32 = 15;
pub const SET_BLOCKLEN: u32 = 16;
pub const READ_SINGLE_BLOCK: u32 = 17;
pub const READ_MULTIPLE_BLOCK: u32 = 18;
pub const SEND_TUNING_BLOCK: u32 = 19;
pub const SPEED_CLASS_CONTROL: u32 = 20;
pub const SET_BLOCK_COUNT: u32 = 23;
pub const WRITE_BLOCK: u32 = 24;
pub const WRITE_MULTIPLE_BLOCK: u32 = 25;
pub const PROGRAM_CSD: u32 = 27;
pub const SET_WRITE_PROT: u32 = 28;
pub const CLR_WRITE_PROT: u32 = 29;
pub const SEND_WRITE_PROT: u32 = 30;
pub const ERASE_WR_BLK_START: u32 = 32;
pub const ERASE_WR_BLK_END: u32 = 33;
pub const ERASE: u32 = 38;
pub const LOCK_UNLOCK: u32 = 42;
pub const APP_CMD: u32 = 55;
pub const GEN_CMD: u32 = 56;

pub const IS_APP_CMD: u32 = 0x8000_0000;

pub const SD_RESET_CMD: u32 = 1 << 25;
pub const SD_RESET_DAT: u32 = 1 << 26;
pub const SD_RESET_ALL: u32 = 1 << 24;

pub const SD_GET_CLOCK_DIVIDER_FAIL: u32 = 0xffff_ffff;

pub const fn acmd(a: u32) -> u32 {
    a | IS_APP_CMD
}

pub const SET_BUS_WIDTH: u32 = acmd(6);
pub const SD_STATUS: u32 = acmd(13);
pub const SEND_NUM_WR_BLOCKS: u32 = acmd(22);
pub const SET_WR_BLK_ERASE_COUNT: u32 = acmd(23);
pub const SD_SEND_OP_COND: u32 = acmd(41);
pub const SET_CLR_CARD_DETECT: u32 = acmd(42);
pub const SEND_SCR: u32 = acmd(51);

pub const SD_BLOCK_SIZE: u32 = 512;

macro_rules! sderror {
    ($msg:expr) => {{
        return Err(SdCardError::Other($msg));
    }};
}

#[derive(Debug)]
pub enum SdCardError {
    Mailbox(mailbox::MailboxError),
    Error,
    CardResetError,
    Other(&'static str),
}

pub struct bcm2711_emmc2_driver {
    base_addr: *mut (),
    emmc: EmmcBlockDev,
}

impl bcm2711_emmc2_driver {
    pub unsafe fn init(base_addr: *mut ()) -> Self {
        let scr_box = Box::new(SdScr {
            scr: [0; 2],
            sd_bus_widths: 0,
            sd_version: 0,
        });

        let emmc = EmmcBlockDev {
            ullOffset: 0,
            hci_ver: 0,
            id: [0; 4],
            csd: [0; 4],
            capacity: u64::MAX,
            card_supports_sdhc: 0,
            card_supports_hs: 0,
            card_supports_18v: 0,
            card_ocr: 0,
            card_rca: CARD_RCA_INVALID,
            last_interrupt: 0,
            last_error: 0,
            scr: Box::into_raw(scr_box),
            failed_voltage_switch: 0,
            last_cmd_reg: 0,
            last_cmd: 0,
            last_cmd_success: 0,
            last_r0: 0,
            last_r1: 0,
            last_r2: 0,
            last_r3: 0,
            buf: core::ptr::null_mut(),
            blocks_to_transfer: 0,
            block_size: SD_BLOCK_SIZE,
            use_sdma: 0,
            card_removal: 0,
            base_clock: 0,
        };

        let mut driver = Self { base_addr, emmc };

        // let mut gpio = GPIO.get().lock();
        // for x in 0..6 {
        //     gpio.set_function(34 + x, gpio::GpioFunction::Input);
        //     gpio.set_function(48 + x, gpio::GpioFunction::Alt3);
        // }

        if let Err(e) = driver.initialize() {
            println!("SDHCI: init_card() failed: {:?}", e);
            panic!("Cannot init SD card");
        }

        driver
    }

    fn reg(&mut self, offset: usize) -> Volatile<u32> {
        Volatile(self.base_addr.wrapping_byte_add(offset).cast::<u32>())
    }

    fn initialize(&mut self) -> Result<(), SdCardError> {
        // Set onboard led status, external gpio pin 132. For hardware
        // https://github.com/raspberrypi/firmware/wiki/Mailbox-property-interface
        // https://forums.raspberrypi.com/viewtopic.php?t=308089 - seems to be related to power supply
        const BUFFER_WORDS: usize = 8;
        let mut buffer = [0u128; BUFFER_WORDS / 4];
        let words: &mut [u32; BUFFER_WORDS] = bytemuck::cast_slice_mut::<_, u32>(&mut buffer)
            .try_into()
            .unwrap();
        // let buffer_size = (words.len() * size_of::<u32>()) as u32;
        let data: [u32; 8] = [32, 0, 0x00038041, 0x8, 0x8, 132, 0, 0];
        words[..data.len()].copy_from_slice(&data);
        unsafe {
            if MAILBOX.get().lock().mailbox_call(8, &mut buffer).is_err() {
                sderror!("Error with mailbox call for sd initialization")
            }
        }

        // let words: &[u32; BUFFER_WORDS] =
        //     bytemuck::cast_slice::<_, u32>(&buffer).try_into().unwrap();
        // println!("{:?}", words);
        // TODO: error handling
        // let response: u32 = words[1];
        // println!("Mailbox response: {:#010x}", response);
        // println!("Pin #: {}", words[5]);
        // println!("Status: {}", words[6] & 0x3);
        if self.card_init().is_err() {
            sderror!("Card initialization failed")
        }
        Ok(())
    }

    fn power_on(&mut self) -> Result<(), SdCardError> {
        self.power(0x3)
    }

    fn power_off(&mut self) -> Result<(), SdCardError> {
        self.power(0x2)
    }

    /// State = 0x3 for power on, 0x2 for power off
    fn power(&mut self, state: u32) -> Result<(), SdCardError> {
        let msg = PropSetPowerState {
            device_id: 0x0,
            state,
        };
        let resp;
        {
            let mut mailbox = MAILBOX.get().lock();
            resp = unsafe { mailbox.get_property::<PropSetPowerState>(msg) };
        }

        if resp.is_err() {
            sderror!("Error with mailbox call for power state")
        }

        Ok(())
    }

    pub fn sd_power_off(&mut self) {
        unsafe {
            let mut control_0 = self.reg(CONTROL0).read();
            control_0 &= !(1 << 8);
            self.reg(CONTROL0).write(control_0);
        }
    }

    fn get_base_clock(&mut self) -> Result<u32, SdCardError> {
        const BUFFER_WORDS: usize = 8;
        let mut buffer = [0u128; BUFFER_WORDS / 4];
        let words: &mut [u32; BUFFER_WORDS] = bytemuck::cast_slice_mut::<_, u32>(&mut buffer)
            .try_into()
            .unwrap();
        // let buffer_size = (words.len() * size_of::<u32>()) as u32;
        let data: [u32; 8] = [32, 0, 0x00030002, 0x8, 0x4, 0xc, 0, 0];
        words[..data.len()].copy_from_slice(&data);
        unsafe {
            if MAILBOX.get().lock().mailbox_call(8, &mut buffer).is_err() {
                sderror!("Error getting base clock rate for SD Card from mailbox call")
            }
        }

        let words: &[u32; BUFFER_WORDS] =
            bytemuck::cast_slice::<_, u32>(&buffer).try_into().unwrap();
        // println!("{:?}", words);
        let clock = words[6];
        // println!("Mailbox response: {:#010x}", words[1]);
        // println!("EMMC device id: {}", words[5]);
        // println!("Clock value {}", words[6]);
        Ok(clock)
    }

    fn get_clock_divider(&mut self, base_clock: u32, target_rate: u32) -> u32 {
        let mut targetted_divisor: u32 = 1;
        if target_rate <= base_clock {
            targetted_divisor = base_clock / target_rate;
            if base_clock % target_rate != 0 {
                targetted_divisor -= 1;
            }
        }

        fn ceil_ilog2(x: u32) -> u32 {
            // Calculate log2 of the next power of 2 >= x
            // (returns 1 for x = 0 and x = 1, for simplicity)
            (x.max(2).saturating_sub(1)).ilog2() + 1
        }
        fn get_divisor(target: u32) -> i32 {
            if target == 0 {
                return 31; // or panic/error?
            } else if target == 1 {
                return 0;
            }
            ceil_ilog2(target).min(31) as i32
        }

        let divisor = get_divisor(targetted_divisor);

        let divisor_u32 = divisor as u32;

        let freq_select: u32 = divisor_u32 & 0xff;
        let upper_bits: u32 = (divisor_u32 >> 8) & 0x3;
        let ret: u32 = (freq_select << 8) | (upper_bits << 6) | (0 << 5);

        ret
    }

    fn switch_clock_rate(&mut self, base_clock: u32, target_rate: u32) -> Result<(), SdCardError> {
        let divider = self.get_clock_divider(base_clock, target_rate);
        if divider == SD_GET_CLOCK_DIVIDER_FAIL {
            return Err(SdCardError::Error);
        }

        while (unsafe { self.reg(STATUS).read() } & 0x3 != 0) {
            spin_sleep(1_000);
        }

        let mut control1 = unsafe { self.reg(CONTROL1).read() };
        control1 &= !(1 << 2);
        unsafe { self.reg(CONTROL1).write(control1) };
        spin_sleep(2_000);

        control1 &= !0xffe0;
        control1 |= divider;
        unsafe { self.reg(CONTROL1).write(control1) };
        spin_sleep(2_000);

        control1 |= 1 << 2;
        unsafe { self.reg(CONTROL1).write(control1) };
        spin_sleep(2_000);

        Ok(())
    }

    fn reset_cmd(&mut self) -> Result<(), SdCardError> {
        let mut control1 = unsafe { self.reg(CONTROL1).read() };
        control1 |= SD_RESET_CMD;
        unsafe { self.reg(CONTROL1).write(control1) };
        self.timeout_wait(CONTROL1, SD_RESET_CMD, 0, 1000000)
    }

    fn reset_dat(&mut self) -> Result<(), SdCardError> {
        let mut control1 = unsafe { self.reg(CONTROL1).read() };
        control1 |= SD_RESET_DAT;
        unsafe { self.reg(CONTROL1).write(control1) };
        self.timeout_wait(CONTROL1, SD_RESET_DAT, 0, 1000000)
    }

    fn timeout_wait(
        &mut self,
        reg: usize,
        mask: u32,
        value: u32,
        usec: u32,
    ) -> Result<(), SdCardError> {
        let start_ticks = get_time_ticks();
        const CLOCKHZ: u32 = 1000000;
        let timeout_ticks: usize = (usec * (CLOCKHZ / 1000000)) as usize;
        while (unsafe { self.reg(reg).read() } & mask != 0) != (value != 0) {
            if get_time_ticks() - start_ticks >= timeout_ticks {
                // println!("{:#010x}", unsafe {self.reg(reg).read()});
                // println!("{:#010x}", mask);
                // println!("{:#010x}", unsafe {self.reg(reg).read() & mask});
                println!("timeout");
                return Err(SdCardError::Error);
            }
        }
        Ok(())
    }

    //TODO: dma?
    fn issue_command_int(
        &mut self,
        cmd_reg: u32,
        argument: u32,
        timeout: u32,
    ) -> Result<(), SdCardError> {
        self.emmc.last_cmd_reg = cmd_reg;
        self.emmc.last_cmd_success = 0;

        // println!("cmd_reg: {:#010x}", cmd_reg);

        // while (unsafe { self.reg(STATUS).read() } & 0x1 != 0) {
        //     spin_sleep(1000);
        // }

        // if cmd_reg & SD_CMD_RSPNS_TYPE_MASK == SD_CMD_RSPNS_TYPE_48B {
        //     if cmd_reg & SD_CMD_RSPNS_TYPE_MASK != SD_CMD_TYPE_ABORT {
        //         while (unsafe { self.reg(STATUS).read() } & 0x2 != 0) {
        //             spin_sleep(1000);
        //         }
        //     }
        // }

        if self.emmc.blocks_to_transfer > 0xffff {
            self.emmc.last_cmd_success = 0;
            println!(
                "blocks_to_transfer too great {}",
                self.emmc.blocks_to_transfer
            );
            return Err(SdCardError::Error);
        }
        // unsafe {println!("interrupt register pre sending command: {:#010x}", self.reg(INTERRUPT).read());}
        let blksizecnt: u32 =
            self.emmc.block_size as u32 | (self.emmc.blocks_to_transfer << 16) as u32;
        // println!("blksizecnt: {:#010x}", blksizecnt);
        unsafe {
            self.reg(BLKSIZECNT).write(blksizecnt);
            // unsafe {println!("interrupt register post blksizecnt: {:#010x}", self.reg(INTERRUPT).read());}
            self.reg(ARG1).write(argument);
            // unsafe {println!("interrupt register post argument: {:#010x}", self.reg(INTERRUPT).read());}
            self.reg(CMDTM).write(cmd_reg);
            // unsafe {println!("interrupt register post cmdtm: {:#010x}", self.reg(INTERRUPT).read());}
        }
        spin_sleep(500000);
        let _ = self.timeout_wait(INTERRUPT, 0x8001, 1, timeout);
        let mut irpts: u32 = unsafe { self.reg(INTERRUPT).read() };
        unsafe { self.reg(INTERRUPT).write(0xffff0001) };
        if irpts & 0xffff0001 != 1 {
            self.emmc.last_error = irpts & 0xffff0000;
            self.emmc.last_interrupt = irpts;
            println!("Error post sending command, irpts: {:#010x}, cmd_reg: {:#010x}, argument: {:#010x}", irpts, cmd_reg, argument);
            return Err(SdCardError::Error);
        }
        unsafe {
            match cmd_reg & SD_CMD_RSPNS_TYPE_MASK {
                SD_CMD_RSPNS_TYPE_48 | SD_CMD_RSPNS_TYPE_48B => {
                    self.emmc.last_r0 = self.reg(RESP0).read();
                }
                SD_CMD_RSPNS_TYPE_136 => {
                    self.emmc.last_r0 = self.reg(RESP0).read();
                    self.emmc.last_r1 = self.reg(RESP1).read();
                    self.emmc.last_r2 = self.reg(RESP2).read();
                    self.emmc.last_r3 = self.reg(RESP3).read();
                }
                _ => {
                    // Do nothing (default case)
                }
            }
        }

        if cmd_reg & SD_CMD_ISDATA != 0 {
            let wr_irpt: u32;
            let mut is_write: i32 = 0;
            if cmd_reg & SD_CMD_DAT_DIR_CH != 0 {
                wr_irpt = 1 << 5;
            } else {
                is_write = 1;
                wr_irpt = 1 << 4;
            }

            assert!(self.emmc.buf.addr() & 3 == 0);
            let mut pData = self.emmc.buf as *mut u32;

            for _ in 0..self.emmc.blocks_to_transfer {
                let _ = self.timeout_wait(INTERRUPT, wr_irpt | 0x8000, 1, timeout as u32);
                unsafe {
                    irpts = self.reg(INTERRUPT).read();
                    self.reg(INTERRUPT).write(0xffff0000 | wr_irpt);
                }
                if irpts & (0xffff0000 | wr_irpt) != wr_irpt {
                    self.emmc.last_error = irpts & 0xffff0000;
                    self.emmc.last_interrupt = irpts;
                    // println!("returning 1019");
                    return Err(SdCardError::Error);
                }

                assert!(self.emmc.block_size <= 1024);
                let mut length: u32 = self.emmc.block_size;
                assert!(length & 3 == 0);
                unsafe {
                    if is_write != 0 {
                        while length > 0 {
                            self.reg(DATA).write(*pData);
                            pData = pData.add(1);
                            length -= 4;
                        }
                    } else {
                        while length > 0 {
                            *pData = self.reg(DATA).read();
                            pData = pData.add(1);
                            length -= 4;
                        }
                    }
                }
            }
        }

        if (cmd_reg & SD_CMD_RSPNS_TYPE_MASK == SD_CMD_RSPNS_TYPE_48B)
            || (cmd_reg & SD_CMD_ISDATA != 0)
        {
            unsafe {
                let _ = self.timeout_wait(INTERRUPT, 0x8002, 1, timeout);
                irpts = self.reg(INTERRUPT).read();
                self.reg(INTERRUPT).write(0xffff0002);

                if (irpts & 0xffff0002 != 2) && (irpts & 0xffff0002 != 0x1000002) {
                    println!("Error occured whilst waiting for transfer complete interrupt");
                    self.emmc.last_error = irpts & 0xffff0000;
                    self.emmc.last_interrupt = irpts;
                    // println!("returning 1054");
                    return Err(SdCardError::Error);
                }

                self.reg(INTERRUPT).write(0xffff0002);
            }
        }

        self.emmc.last_cmd_success = 1;
        Ok(())
    }

    fn handle_card_interrupt(&mut self) {
        if self.emmc.card_rca != CARD_RCA_INVALID {
            let _ = self.issue_command_int(
                sd_commands[SEND_STATUS as usize],
                self.emmc.card_rca << 16,
                500000,
            );
            if self.emmc.fail() {
                println!("Unable to get card status");
            }
        }
    }

    fn handle_interrupts(&mut self) {
        let irpts = unsafe { self.reg(INTERRUPT).read() };
        // println!("handle_interupts irpts: {:#010x}", irpts);
        let mut reset_mask: u32 = 0;

        if irpts & SD_COMMAND_COMPLETE != 0 {
            reset_mask |= SD_COMMAND_COMPLETE;
        }

        if irpts & SD_TRANSFER_COMPLETE != 0 {
            reset_mask |= SD_TRANSFER_COMPLETE;
        }

        if irpts & SD_BLOCK_GAP_EVENT != 0 {
            reset_mask |= SD_BLOCK_GAP_EVENT;
        }

        if irpts & SD_DMA_INTERRUPT != 0 {
            reset_mask |= SD_DMA_INTERRUPT;
        }

        if irpts & SD_BUFFER_WRITE_READY != 0 {
            reset_mask |= SD_BUFFER_WRITE_READY;
            let _ = self.reset_dat();
        }

        if irpts & SD_BUFFER_READ_READY != 0 {
            reset_mask |= SD_BUFFER_READ_READY;
            let _ = self.reset_dat();
        }

        if irpts & SD_CARD_INSERTION != 0 {
            reset_mask |= SD_CARD_INSERTION;
        }

        if irpts & SD_CARD_REMOVAL != 0 {
            reset_mask |= SD_CARD_REMOVAL;
            self.emmc.card_removal = 1;
        }

        if irpts & SD_CARD_INTERRUPT != 0 {
            self.handle_card_interrupt();
            reset_mask |= SD_CARD_INTERRUPT;
        }

        if irpts & 0x8000 != 0 {
            reset_mask |= 0xffff0000;
        }
        // println!("Reset mask: {:#010x}", reset_mask);
        unsafe { self.reg(INTERRUPT).write(reset_mask) };
    }

    fn issue_command(
        &mut self,
        mut command: u32,
        argument: u32,
        timeout: u32,
    ) -> Result<(), SdCardError> {
        self.handle_interrupts();
        // unsafe {println!("interrupt register post handle_interrupts: {:#010x}", self.reg(INTERRUPT).read());}
        // println!("command: {:#010x}", command);
        if self.emmc.card_removal != 0 {
            self.emmc.last_cmd_success = 0;
            return Err(SdCardError::Error);
        }

        if command & IS_APP_CMD != 0 {
            // println!("APP_CMD command being sent");
            command &= 0xff;
            // println!("Command hex value {:#010x}", command);
            if sd_acommands[command as usize] == sd_cmd_reserved(0) {
                println!("Invalid command ACMD{command}");
                self.emmc.last_cmd_success = 0;
                return Err(SdCardError::Error);
            }
            self.emmc.last_cmd = APP_CMD;

            let mut rca: u32 = 0;
            if self.emmc.card_rca != CARD_RCA_INVALID {
                rca = self.emmc.card_rca << 16;
            }
            // println!("sd_commands[APP_CMD as usize]: {:#010x}", sd_commands[APP_CMD as usize]);
            let _ = self.issue_command_int(sd_commands[APP_CMD as usize], rca, timeout);
            if self.emmc.last_cmd_success != 0 {
                self.emmc.last_cmd = command | IS_APP_CMD;
                let _ = self.issue_command_int(sd_acommands[command as usize], argument, timeout);
            } else {
                // println!("sd command: {:#010x}", sd_commands[APP_CMD as usize]);
                // println!("rca: {rca}");
            }
        } else {
            if sd_commands[command as usize] == sd_cmd_reserved(0) {
                println!("Invalid command CMD{command}");
                self.emmc.last_cmd_success = 0;
                return Err(SdCardError::Error);
            }

            self.emmc.last_cmd = command;
            let _ = self.issue_command_int(sd_commands[command as usize], argument, timeout);
        }
        if self.emmc.last_cmd_success != 0 {
            Ok(())
        } else {
            println!("Error issuing command {:#010x}", command);
            Err(SdCardError::Error)
        }
    }

    fn get_CSD_field(&mut self, mut start: u32, width: u32) -> u32 {
        assert!(start >= 8);
        start -= 8;

        let offset = (start / 32) as usize;
        let shift = (start & 31) as u32;

        let mut result = self.emmc.csd[offset] >> shift;
        if width + shift > 32 {
            result |= self.emmc.csd[offset + 1] << ((32 - shift) % 32);
        }

        let mask = if width < 32 {
            (1 << width) - 1
        } else {
            0xFFFF_FFFF
        };

        result & mask
    }

    fn card_reset(&mut self) -> Result<(), SdCardError> {
        unsafe {
            let mut control1 = self.reg(CONTROL1).read();
            // println!("control1: {:#010x}", control1);
            control1 |= 1 << 24;
            // println!("{:#010x}", control1);
            control1 &= !(1 << 2);
            // println!("{:#010x}", control1);
            control1 &= !(1 << 0);
            // println!("Writing {:#010X}, to SD:CONTROL1.", control1);
            self.reg(CONTROL1).write(control1);
            if self.timeout_wait(CONTROL1, 7 << 24, 0, 1000000).is_err() {
                println!("Controller did not reset properly");
                return Err(SdCardError::Error);
            }

            spin_sleep(5000);

            let mut control0 = self.reg(CONTROL0).read();
            control0 |= 0x0F << 8;
            // println!("Wrote {:#010x} to SD:CONTROL0.", control0);
            self.reg(CONTROL0).write(control0);
            spin_sleep(2000);

            if self.timeout_wait(STATUS, 1 << 16, 1, 500000).is_err() {
                // println!("Timed out on status check during reset, STATUS:, {:#010x}", self.reg(STATUS).read());
            }
            let mut status_reg = self.reg(STATUS).read();
            if status_reg & (1 << 16) == 0 {
                println!("No card inserted, status: {:010x}", status_reg);
                // return Err(SdCardError::Error);
            }

            self.reg(CONTROL2).write(0);

            let mut base_clock = self.get_base_clock().unwrap();
            // println!("base clock: {base_clock}");
            if base_clock == 0 {
                base_clock = 100000000;
            }

            control1 = self.reg(CONTROL1).read();
            control1 |= 1;

            let f_id = self.get_clock_divider(base_clock, SD_CLOCK_ID);
            // println!("f_id: {:#010x}", f_id);
            if f_id == SD_GET_CLOCK_DIVIDER_FAIL {
                println!("Unable to get a valid clock divider for ID frequency");
                return Err(SdCardError::Error);
            }
            // control1 &= !(0x3ff << 6);
            control1 |= f_id;

            control1 &= !(0xf << 16);

            control1 |= 11 << 16;
            // println!("Writing {:#010X}, to SD:CONTROL1.", control1);
            self.reg(CONTROL1).write(control1);

            if self.timeout_wait(CONTROL1, 2, 1, 1000000).is_err() {
                println!("Clock did not stabilise within 1 second");
                return Err(SdCardError::Error);
            }

            spin_sleep(2000);
            control1 = self.reg(CONTROL1).read();
            control1 |= 4;
            // println!("Writing {:#010X}, to SD:CONTROL1.", control1);
            self.reg(CONTROL1).write(control1);
            spin_sleep(2000);

            self.reg(IRPT_EN).write(0);

            self.reg(INTERRUPT).write(0xffffffff);

            let irpt_mask = 0xffffffff & !(SD_CARD_INTERRUPT);
            // println!("irpt_mask: {:#010x}", irpt_mask);
            self.reg(IRPT_MASK).write(irpt_mask);

            spin_sleep(2000);

            self.emmc.id[0] = 0;
            self.emmc.id[1] = 0;
            self.emmc.id[2] = 0;
            self.emmc.id[3] = 0;

            self.emmc.csd[0] = 0;
            self.emmc.csd[1] = 0;
            self.emmc.csd[2] = 0;
            self.emmc.csd[3] = 0;

            self.emmc.card_supports_sdhc = 0;
            self.emmc.card_supports_hs = 0;
            self.emmc.card_supports_18v = 0;
            self.emmc.card_ocr = 0;
            self.emmc.card_rca = CARD_RCA_INVALID;
            self.emmc.last_interrupt = 0;
            self.emmc.last_error = 0;
            self.emmc.failed_voltage_switch = 0;
            self.emmc.last_cmd_reg = 0;
            self.emmc.last_cmd = 0;
            self.emmc.last_cmd_success = 0;
            self.emmc.last_r0 = 0;
            self.emmc.last_r1 = 0;
            self.emmc.last_r2 = 0;
            self.emmc.last_r3 = 0;

            self.emmc.buf = core::ptr::null_mut();
            self.emmc.blocks_to_transfer = 0;
            self.emmc.block_size = 0;

            self.emmc.card_removal = 0;
            self.emmc.base_clock = 0;

            self.emmc.base_clock = base_clock;

            self.issue_command(GO_IDLE_STATE, 0, 500000)?;

            if self.issue_command(SEND_IF_COND, 0x1aa, 1000000).is_err() {
                println!("CMD8 error");
            }
            let mut v2_later = 0;
            if self.emmc.timeout() {
                v2_later = 0;
            } else if self.emmc.cmd_timeout() {
                if self.reset_cmd().is_err() {
                    return Err(SdCardError::Error);
                }
                self.reg(INTERRUPT).write(SD_ERR_MASK_CMD_TIMEOUT);
                v2_later = 0;
            } else if self.emmc.fail() {
                println!("Failure sending CMD8");
                return Err(SdCardError::Error);
            } else {
                if self.emmc.last_r0 & 0xfff != 0x1aa {
                    println!("Unusable card");
                    println!("CMD8 response {:#010x}", self.emmc.last_r0);
                } else {
                    v2_later = 1;
                }
            }

            // println!("v2_later: {v2_later}");

            if self.issue_command(IO_SET_OP_COND, 0, 1000000).is_err() {
                if !(self.emmc.timeout()) {
                    if self.emmc.cmd_timeout() {
                        if self.reset_cmd().is_err() {
                            return Err(SdCardError::Error);
                        }
                        self.reg(INTERRUPT).write(SD_ERR_MASK_CMD_TIMEOUT);
                    } else {
                        println!("SDIO card detected - not currently supported");
                        return Err(SdCardError::Error);
                    }
                }
            }

            if let Err(e) = self.issue_command(acmd(41), 0, 1000000) {
                println!("Inquiry ACMD41 failed");
                // spin_sleep(10000000000000);
                return Err(e);
            }

            let mut card_is_busy = 1;
            while card_is_busy != 0 {
                let mut v2_flags: u32 = 0;
                if v2_later != 0 {
                    v2_flags |= 1 << 30;
                }

                v2_flags |= 1 << 28;

                if let Err(e) = self.issue_command(acmd(41), 0x00ff8000 | v2_flags, 500000) {
                    println!("Error issuing ACMD41");
                    return Err(e);
                }

                if self.emmc.last_r0 >> 31 & 1 != 0 {
                    self.emmc.card_ocr = (self.emmc.last_r0 >> 8) & 0xffff;
                    self.emmc.card_supports_sdhc = (self.emmc.last_r0 >> 30) & 0x1;

                    card_is_busy = 0;
                } else {
                    spin_sleep(500000);
                }
            }

            let _ = self.switch_clock_rate(base_clock, SD_CLOCK_NORMAL);

            spin_sleep(5000);

            if self.emmc.card_supports_18v != 0 {
                if self.issue_command(VOLTAGE_SWITCH, 0, 500000).is_err() {
                    println!("Error issuing VOLTAGE_SWITCH");
                    self.emmc.failed_voltage_switch = 1;
                    self.sd_power_off();
                    return self.card_reset();
                }

                control1 = self.reg(CONTROL1).read();
                control1 &= !(1 << 2);
                self.reg(CONTROL1).write(control1);

                status_reg = self.reg(STATUS).read();
                let mut dat30: u32 = (status_reg >> 20) & 0xf;
                if dat30 != 0 {
                    println!("DAT[3:0] did not settle to 0");
                    self.emmc.failed_voltage_switch = 1;
                    self.sd_power_off();
                    return self.card_reset();
                }

                control0 = self.reg(CONTROL0).read();
                control0 |= 1 << 8;
                self.reg(CONTROL0).write(control0);

                spin_sleep(5000);

                control0 = self.reg(CONTROL0).read();
                if (control0 >> 8) & 1 == 0 {
                    println!("Controller did not keep 1.8V signal enable high");
                    self.emmc.failed_voltage_switch = 1;
                    self.sd_power_off();
                    return self.card_reset();
                }

                control1 = self.reg(CONTROL1).read();
                control1 |= 1 << 2;
                self.reg(CONTROL1).write(control1);

                spin_sleep(5000);

                status_reg = self.reg(STATUS).read();
                dat30 = (status_reg >> 20) & 0xf;
                if dat30 != 0xf {
                    println!("DAT[3:0] did not settle to 1111b {:#010x}", dat30);
                    self.emmc.failed_voltage_switch = 1;
                    self.sd_power_off();
                    return self.card_reset();
                }
            }

            if let Err(e) = self.issue_command(ALL_SEND_CID, 0, 500000) {
                println!("Error sending ALL_SEND_CID");
                return Err(e);
            }

            self.emmc.id[0] = self.emmc.last_r0;
            self.emmc.id[1] = self.emmc.last_r1;
            self.emmc.id[2] = self.emmc.last_r2;
            self.emmc.id[3] = self.emmc.last_r3;

            if let Err(e) = self.issue_command(SEND_RELATIVE_ADDR, 0, 500000) {
                println!("Error sending SEND_RELATIVE_ADDR");
                return Err(e);
            }

            let cmd3_resp: u32 = self.emmc.last_r0;

            self.emmc.card_rca = (cmd3_resp >> 16) & 0xffff;
            let crc_error = ((cmd3_resp >> 15) & 0x1) != 0;
            let illegal_cmd = ((cmd3_resp >> 14) & 0x1) != 0;
            let error_flag = ((cmd3_resp >> 13) & 0x1) != 0;
            // let mut status = (cmd3_resp >> 9) & 0x0f; // 4 bits
            let ready = ((cmd3_resp >> 8) & 0x1) != 0;

            if crc_error {
                sderror!("CRC error")
            }

            if illegal_cmd {
                sderror!("Illegal command")
            }

            if error_flag {
                sderror!("Generic error")
            }

            if !ready {
                sderror!("Not ready for data")
            }

            if let Err(e) = self.issue_command(SEND_CSD, self.emmc.card_rca << 16, 500000) {
                println!("Error sending CMD9");
                return Err(e);
            }
            self.emmc.csd[0] = self.emmc.last_r0;
            self.emmc.csd[1] = self.emmc.last_r1;
            self.emmc.csd[2] = self.emmc.last_r2;
            self.emmc.csd[3] = self.emmc.last_r3;

            let nSize: u32;
            let nShift: u32;
            match self.get_CSD_field(126, 2) {
                0 => {
                    nSize = self.get_CSD_field(62, 12) + 1;
                    nShift = self.get_CSD_field(47, 3) + 2;
                }
                1 => {
                    nSize = self.get_CSD_field(48, 22) + 1;
                    nShift = 10;
                }
                _ => {
                    sderror!("Unknown CSD version")
                }
            }

            self.emmc.capacity = (nSize << nShift) as u64 * SD_BLOCK_SIZE as u64;
            println!("SD capacity is {:?} Bytes", self.emmc.capacity);

            if let Err(e) = self.issue_command(SELECT_CARD, self.emmc.card_rca << 16, 500000) {
                println!("Error sending CMD7");
                return Err(e);
            }

            let cmd7_reso = self.emmc.last_r0;
            let status = (cmd7_reso >> 9) & 0xf;
            if status != 3 && status != 4 {
                sderror!("Invalid status")
            }

            if self.emmc.card_supports_sdhc == 0 {
                if let Err(e) = self.issue_command(SET_BLOCKLEN, SD_BLOCK_SIZE, 500000) {
                    println!("Error sending SET_BLOCKLEN");
                    return Err(e);
                }
            }

            let mut controller_block_size = self.reg(BLKSIZECNT).read();
            controller_block_size &= !(0xfff);
            controller_block_size |= 0x200;
            self.reg(BLKSIZECNT).write(controller_block_size);

            // let scr_ref: &mut SdScr = &mut *self.emmc.scr;
            // self.emmc.buf = &mut scr_ref.scr[0] as *mut u32 as *mut ();

            self.emmc.buf = (*self.emmc.scr).scr.as_mut_ptr() as *mut ();

            self.emmc.block_size = 8;
            self.emmc.blocks_to_transfer = 1;
            let _ = self.issue_command(SEND_SCR, 0, 1000000);
            self.emmc.block_size = SD_BLOCK_SIZE;
            if self.emmc.fail() {
                return Err(SdCardError::CardResetError);
            }

            let scr0: u32 = (*self.emmc.scr).scr[0].swap_bytes();
            let sd_spec = (scr0 >> (56 - 32)) & 0xf;
            let sd_spec3 = (scr0 >> (47 - 32)) & 0x1;
            let sd_spec4 = (scr0 >> (42 - 32)) & 0x1;
            (*self.emmc.scr).sd_bus_widths = (scr0 >> (48 - 32)) & 0xf;
            let mut sd_version = SD_VER_UNKNOWN;
            if sd_spec == 0 {
                sd_version = SD_VER_1;
            } else if sd_spec == 1 {
                sd_version = SD_VER_1_1;
            } else if sd_spec == 2 {
                if sd_spec3 == 0 {
                    sd_version = SD_VER_2;
                } else if sd_spec3 == 1 {
                    if sd_spec4 == 0 {
                        sd_version = SD_VER_3;
                    } else if sd_spec4 == 1 {
                        sd_version = SD_VER_4;
                    }
                }
            }
            (*self.emmc.scr).sd_version = sd_version;

            if (*self.emmc.scr).sd_version >= SD_VER_1_1 {
                let mut cmd6_resp = [0u8; 64];
                self.emmc.buf = cmd6_resp.as_mut_ptr() as *mut ();
                self.emmc.block_size = 64;

                if self.issue_command(SWITCH_FUNC, 0x00fffff0, 100000).is_err() {
                    println!("Error sending SWITCH_FUNC (Mode 0)");
                } else {
                    self.emmc.card_supports_hs = (cmd6_resp[13] >> 1) as u32 & 0x1;
                    if self.emmc.card_supports_hs != 0 {
                        if self.issue_command(SWITCH_FUNC, 0x80fffff1, 100000).is_err() {
                            println!("Switch failed");
                        } else {
                            let _ = self.switch_clock_rate(base_clock, SD_CLOCK_HIGH);
                        }
                    }
                }
                self.emmc.block_size = SD_BLOCK_SIZE;
            }

            if (*self.emmc.scr).sd_bus_widths & 4 != 0 {
                let old_irpt_mask = self.reg(IRPT_MASK).read();
                let new_irpt_mask = old_irpt_mask & !(1 << 8);
                self.reg(IRPT_MASK).write(new_irpt_mask);

                if self.issue_command(SET_BUS_WIDTH, 2, 500000).is_err() {
                    println!("Switch to 4-bit data mode failed");
                } else {
                    let mut control0 = self.reg(CONTROL0).read();
                    control0 |= 0x2;
                    self.reg(CONTROL0).write(control0);
                    self.reg(IRPT_MASK).write(old_irpt_mask);
                }
            }

            println!(
                "Found a valid {:#?} SD Card",
                SD_VERSIONS[(*self.emmc.scr).sd_version as usize]
            );

            self.reg(INTERRUPT).write(0xffffffff);
        }
        Ok(())
    }

    fn card_init(&mut self) -> Result<(), SdCardError> {
        if self.power_on().is_err() {
            sderror!("SD Card Controller did not power on successfully")
        }

        spin_sleep(5000);
        assert!(size_of_val(&sd_commands) == 64 * size_of::<u32>());
        assert!(size_of_val(&sd_acommands) == 64 * size_of::<u32>());

        let ver = unsafe { self.reg(SLOTISR_VER).read() };
        let sdversion = (ver >> 16) & 0xff;
        let vendor = ver >> 24;
        let slot_status = ver & 0xff;
        println!(
            "SD version: {:#?}, vendor: {:#010x}, slot status: {:#010x}",
            SD_VERSIONS[sdversion as usize], vendor, slot_status
        );

        self.emmc.hci_ver = sdversion;
        if self.emmc.hci_ver < 2 {
            println!("Old SDHCI version detected");
        }

        let mut ret: Result<(), SdCardError> = Ok(());
        for _ in 0..3 {
            ret = self.card_reset();
            match ret {
                Err(SdCardError::CardResetError) => println!("Card reset failed. Retrying."),
                _ => break,
            }
        }

        return ret;
    }

    fn ensure_data_mode(&mut self) -> Result<(), SdCardError> {
        if self.emmc.card_rca == CARD_RCA_INVALID {
            let ret = self.card_reset();
            if ret.is_err() {
                return ret;
            }
        }

        if let Err(e) = self.issue_command(SEND_STATUS, self.emmc.card_rca << 16, 500000) {
            self.emmc.card_rca = CARD_RCA_INVALID;
            println!("ensure_data_mode() error sending CMD13");
            return Err(e);
        }

        let mut status: u32 = self.emmc.last_r0;
        let mut cur_state = (status >> 9) & 0xf;

        if cur_state == 3 {
            if let Err(e) = self.issue_command(SELECT_CARD, self.emmc.card_rca << 16, 500000) {
                self.emmc.card_rca = CARD_RCA_INVALID;
                println!("ensure_data_mode() no response from CMD17");
                return Err(e);
            }
        } else if cur_state == 5 {
            if let Err(e) = self.issue_command(STOP_TRANSMISSION, 0, 500000) {
                self.emmc.card_rca = CARD_RCA_INVALID;
                println!("ensure_data_mode() no response from CMD12");
                return Err(e);
            }
            let _ = self.reset_dat();
        } else if cur_state != 4 {
            let ret = self.card_reset();
            if ret.is_err() {
                return ret;
            }
        }
        if cur_state != 4 {
            if let Err(e) = self.issue_command(SEND_STATUS, self.emmc.card_rca << 16, 500000) {
                self.emmc.card_rca = CARD_RCA_INVALID;
                println!("ensure_data_mode() error sending CMD13");
                return Err(e);
            }
            status = self.emmc.last_r0;
            cur_state = (status >> 9) & 0xf;
            if cur_state != 4 {
                self.emmc.card_rca = CARD_RCA_INVALID;
                sderror!("Unable to initialize SD card to date mode")
            }
        }
        Ok(())
    }

    fn do_data_command(
        &mut self,
        is_write: bool,
        buf: &mut [u8],
        mut block_no: u32,
    ) -> Result<(), SdCardError> {
        if self.emmc.card_supports_sdhc == 0 {
            block_no *= SD_BLOCK_SIZE;
        }

        if buf.len() < self.emmc.block_size as usize {
            sderror!("do_data_command() called with buffer size less than block size")
        }

        self.emmc.blocks_to_transfer = (buf.len() / self.emmc.block_size as usize) as i32;
        if buf.len() % self.emmc.block_size as usize != 0 {
            sderror!(
                "do_data_command() called with buffer size not an exact multiple of block size"
            )
        }

        self.emmc.buf = buf.as_mut_ptr() as *mut ();
        let command: u32;
        if is_write {
            if self.emmc.blocks_to_transfer > 1 {
                command = WRITE_MULTIPLE_BLOCK;
            } else {
                command = WRITE_BLOCK;
            }
        } else {
            if self.emmc.blocks_to_transfer > 1 {
                command = READ_MULTIPLE_BLOCK;
            } else {
                command = READ_SINGLE_BLOCK;
            }
        }

        let mut retry_count = 0;
        let max_retries = 3;
        while retry_count < max_retries {
            if self.issue_command(command, block_no, 500000).is_ok() {
                break;
            } else {
                println!("Error sending CMD{command}");
                println!("error = {:08x}", self.emmc.last_error);

                retry_count += 1;
                if retry_count < max_retries {
                    println!("Retrying");
                } else {
                    println!("Giving up");
                }
            }
        }

        if retry_count == max_retries {
            self.emmc.card_rca = CARD_RCA_INVALID;
            return Err(SdCardError::Error);
        }
        Ok(())
    }

    fn do_read(&mut self, buf: &mut [u8], block_no: u32) -> Result<u32, SdCardError> {
        self.ensure_data_mode()?;

        self.do_data_command(false, buf, block_no)?;

        Ok(buf.len() as u32)
    }

    fn do_write(&mut self, buf: &[u8], block_no: u32) -> Result<u32, SdCardError> {
        if self.ensure_data_mode().is_err() {
            return Err(SdCardError::Error);
        }
        let ptr = buf.as_ptr() as *mut u8;
        let tmp: &mut [u8] = unsafe { core::slice::from_raw_parts_mut(ptr, buf.len()) };
        self.do_data_command(true, tmp, block_no)?;

        Ok(buf.len() as u32)
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<u32, SdCardError> {
        if self.emmc.ullOffset % SD_BLOCK_SIZE as u64 != 0 {
            sderror!("read() called with offset not a multiple of block size")
        }

        let nBlock: u32 = self.emmc.ullOffset as u32 / SD_BLOCK_SIZE as u32;

        let amount_read = self.do_read(buf, nBlock)?;
        if amount_read as usize != buf.len() {
            sderror!("read() returned value different than requested bytes");
        }

        Ok(buf.len() as u32)
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<u32, SdCardError> {
        if self.emmc.ullOffset % SD_BLOCK_SIZE as u64 != 0 {
            sderror!("write() called with offset not a multiple of block size")
        }

        let nBlock: u32 = self.emmc.ullOffset as u32 / SD_BLOCK_SIZE as u32;

        let amount_written = self.do_write(buf, nBlock)?;
        if amount_written as usize != buf.len() {
            sderror!("read() returned value different than requested bytes");
        }

        Ok(buf.len() as u32)
    }

    pub fn seek(&mut self, offset: u64) -> u64 {
        self.emmc.ullOffset = offset;
        self.emmc.ullOffset
    }

    pub fn get_capacity(&self) -> u64 {
        self.emmc.capacity
    }

    pub fn get_block_size(&self) -> u32 {
        self.emmc.block_size
    }

    pub fn get_id(&self) -> &[u32; 4] {
        &self.emmc.id
    }

}

impl BlockDevice for bcm2711_emmc2_driver {
    fn read_sector(
        &mut self,
        index: u64,
        buffer: &mut [u8; filesystem::SECTOR_SIZE],
    ) -> Result<(), filesystem::BlockDeviceError> {
        self.seek(index * filesystem::SECTOR_SIZE as u64);
        self.read(buffer)
            .map_err(|_| filesystem::BlockDeviceError::Unknown)?;
        Ok(())
    }

    fn write_sector(
        &mut self,
        index: u64,
        buffer: &[u8; filesystem::SECTOR_SIZE],
    ) -> Result<(), filesystem::BlockDeviceError> {
        self.seek(index * filesystem::SECTOR_SIZE as u64);
        self.write(buffer)
            .map_err(|_| filesystem::BlockDeviceError::Unknown)?;
        Ok(())
    }

    fn read_sectors(
        &mut self,
        index: u64,
        buffer: &mut [u8],
    ) -> Result<(), filesystem::BlockDeviceError> {
        self.seek(index * filesystem::SECTOR_SIZE as u64);
        self.read(buffer)
            .map_err(|_| filesystem::BlockDeviceError::Unknown)?;
        Ok(())
    }
    fn write_sectors(
        &mut self,
        index: u64,
        buffer: &[u8],
    ) -> Result<(), filesystem::BlockDeviceError> {
        self.seek(index * filesystem::SECTOR_SIZE as u64);
        self.write(buffer)
            .map_err(|_| filesystem::BlockDeviceError::Unknown)?;
        Ok(())
    }
}

unsafe impl Send for bcm2711_emmc2_driver {}
unsafe impl Sync for bcm2711_emmc2_driver {}
