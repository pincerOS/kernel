#![allow(dead_code, nonstandard_style)]

// SDHCI driver for raspberry pi 4b
// TODO:
// Move print statements to debug
// Debug what needs to be reset on initialization
// Fix metadata initialization
// Error handling
// DMA? Multi-block reads?

// https://github.com/jncronin/rpi-boot/blob/master/emmc.c

use crate::sync::Volatile;

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

const C1_CLK_INTLEN: u32 = 1 << 0;    // Keep internal clock running even if SD clock disabled
const C1_SRST_DATA: u32 = 1 << 26;    // Reset data lines only
const C1_SRST_CMD:  u32 = 1 << 25;    // Reset command line only
const C1_SRST_HC:   u32 = 1 << 24;    // Reset entire host controller (full reset)

// Possibly 3.3V bits for bus power in CONTROL0, but check actual docs
const SD_BUS_VOLTAGE_3_3V: u32 = 0b111 << 8;
const SD_CLOCK_DIVIDER_400KHZ: u32 = 0x80;

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum SdCommand {
    Cmd0 = 0,
    Cmd2 = 2,
    Cmd3 = 3,
    Cmd7 = 7,
    Cmd8 = 8,
    Cmd9 = 9,
    Cmd16 = 16,
    Cmd17 = 17,
    Cmd18 = 18,
    Cmd24 = 24,
    Cmd25 = 25,
    Acmd41 = 41,
    Cmd55 = 55,
}

pub struct SdCardMetadata {
    cid: [u32; 4],
    csd: [u32; 4],
    capacity: u64,
    rca: u32,
    card_type: &'static str,
}

pub struct bcm2711_sdhci_driver {
    base_addr: *mut (),
    metadata: SdCardMetadata,
}

impl bcm2711_sdhci_driver {
    pub unsafe fn init(base_addr: *mut ()) -> Self {
        let mut driver = Self {
            base_addr,
            metadata: SdCardMetadata {
                cid: [0; 4],
                csd: [0; 4],
                capacity: 0,
                rca: 0,
                card_type: "Unknown",
            },
        };

        if let Err(e) = driver.init_card() {
            println!("SDHCI: init_card() failed: {}", e);
            panic!("Cannot init SD card");
        }
        if let Err(e) = driver.populate_metadata() {
            println!("SDHCI: populate_metadata() failed: {}", e);
            panic!("Cannot read metadata: {}", e);
        }
        driver.print_metadata();
        driver
    }

    fn reg(&self, offset: usize) -> Volatile<u32> {
        Volatile(self.base_addr.wrapping_byte_add(offset).cast::<u32>())
    }

    pub fn init_card(&mut self) -> Result<(), &'static str> {
        println!("SDHCI: init_card() called");
        self.print_capabilities(); // For debugging
        self.reset_full()?;
        self.send_command(SdCommand::Cmd0, 0)?;
        self.send_command(SdCommand::Cmd8, 0x1AA)?;
        // self.send_acmd41_loop()?;
        println!("SDHCI: Card initialization complete");
        Ok(())
    }

    /// Debug function: read and print CAPABILITIES_0/1
    fn print_capabilities(&self) {
        let caps0 = unsafe { self.reg(CAPABILITIES_0).read() };
        let caps1 = unsafe { self.reg(CAPABILITIES_1).read() };
        println!("SDHCI: CAP0=0x{:08X}, CAP1=0x{:08X}", caps0, caps1);
    }

    fn print_metadata(&self) {
        println!("SDHCI: Card Metadata:");
        println!(
            "  CID   = {:08X} {:08X} {:08X} {:08X}",
            self.metadata.cid[0],
            self.metadata.cid[1],
            self.metadata.cid[2],
            self.metadata.cid[3]
        );
        println!(
            "  CSD   = {:08X} {:08X} {:08X} {:08X}",
            self.metadata.csd[0],
            self.metadata.csd[1],
            self.metadata.csd[2],
            self.metadata.csd[3]
        );
        println!("  RCA   = {}", self.metadata.rca);
        println!("  Type  = {}", self.metadata.card_type);
        println!(
            "  Capacity = {} bytes (approx {:.2} MB)",
            self.metadata.capacity,
            self.metadata.capacity as f64 / (1024.0 * 1024.0)
        );
    }

    /// Gathers CID, RCA, CSD, capacity, and card type
    fn populate_metadata(&mut self) -> Result<(), &'static str> {
        println!("SDHCI: populate_metadata()...");
        self.metadata.cid = self.read_cid()?;
        self.metadata.rca = self.get_rca()?;
        self.metadata.csd = self.read_csd(self.metadata.rca)?;
        self.metadata.capacity = self.get_card_capacity(self.metadata.rca);
        self.metadata.card_type = self.detect_card_type(self.metadata.rca);
        println!("SDHCI: populate_metadata() success");
        Ok(())
    }

    fn send_acmd41_loop(&mut self) -> Result<(), &'static str> {
        let timeout = 100;
        for _ in 0..timeout {
            // CMD55
            self.send_command(SdCommand::Cmd55, 0)?;
    
            // ACMD41 with desired OCR bits, e.g., 0x40FF8000
            self.send_command(SdCommand::Acmd41, 0x40FF8000)?;
    
            // Check card’s response in RESP0 for busy bit cleared
            let resp = unsafe { self.reg(RESP0).read() };
            if (resp & (1 << 31)) != 0 {
                // Card is ready
                println!("SDHCI: ACMD41 indicates card is ready");
                return Ok(());
            }
        }
    
        Err("ACMD41 loop timed out: card not ready")
    }
    

    /// Attempt a full reset using partial resets if needed
    fn reset_full(&mut self) -> Result<(), &'static str> {
        // 1) try enabling the internal clock first (C1_CLK_INTLEN)
        let reg_control1 = self.reg(CONTROL1);
        let mut ctrl1 = unsafe { reg_control1.read() };
        ctrl1 |= C1_CLK_INTLEN; 
        unsafe { reg_control1.write(ctrl1) };
        println!("SDHCI: set CLK_INTLEN, CONTROL1=0x{:X}", unsafe { reg_control1.read() });

        // 2) do the normal reset, if that fails, do partial resets
        match self.reset() {
            Ok(_) => {
                println!("SDHCI: full reset success");
                return Ok(());
            }
            Err(e) => {
                println!("SDHCI: full reset failed: {}, trying partial resets", e);
            }
        }

        // partial reset: reset CMD line
        if let Err(e) = self.reset_cmd_line() {
            println!("SDHCI: partial reset_cmd_line() also failed: {}", e);
        } else {
            println!("SDHCI: partial reset CMD line success");
        }

        // partial reset: reset DAT line
        if let Err(e) = self.reset_dat_line() {
            println!("SDHCI: partial reset_dat_line() also failed: {}", e);
        } else {
            println!("SDHCI: partial reset DAT line success");
        }

        // final attempt: do the normal reset again
        match self.reset() {
            Ok(_) => {
                println!("SDHCI: second attempt full reset success");
                Ok(())
            }
            Err(e) => {
                println!("SDHCI: second attempt full reset still fails: {}", e);
                Err(e)
            }
        }
    }

    /// The standard reset approach
    pub fn reset(&mut self) -> Result<(), &'static str> {
        println!("SDHCI: reset() start");
        let reg_status    = self.reg(STATUS);
        let reg_control0  = self.reg(CONTROL0);
        let reg_control1  = self.reg(CONTROL1);

        unsafe {
            // Wait for CMD/DAT Inhibit
            let mut count = 0;
            let timeout = 100_000;
            // while (reg_status.read() & 0x3) != 0 {
            //     if count > timeout {
            //         println!("SDHCI: CMD/DAT Inhibit bits=0x{:X}", reg_status.read());
            //         return Err("CMD/DAT Inhibit never cleared before reset");
            //     }
            //     count += 1;
            // }

            // println!("SDHCI: CMD/DAT Inhibit cleared, CONTROL1=0x{:X}", reg_control1.read());

            // Clear CONTROL1, then set SRST
            reg_control1.write(0);
            reg_control1.write(C1_SRST_HC);

            count = 0;
            while (reg_control1.read() & C1_SRST_HC) != 0 {
                if count > timeout {
                    println!("SDHCI: CONTROL1=0x{:X}", reg_control1.read());
                    return Err("Reset bit never cleared (SRST timeout)");
                }
                count += 1;
            }
            println!("SDHCI: Reset bit cleared, CONTROL1=0x{:X}", reg_control1.read());

            // Bus voltage 3.3V
            let mut c0_val = reg_control0.read();
            c0_val &= !(0x7 << 8);
            c0_val |= SD_BUS_VOLTAGE_3_3V;
            reg_control0.write(c0_val);
            println!("SDHCI: CONTROL0 set to 3.3V=0x{:X}", c0_val);

            // set the ~400 kHz clock
            let mut c1_val = reg_control1.read();
            // SHIFT for divider is bits [15:8]
            c1_val &= !(0xFF << 8);
            c1_val |= (SD_CLOCK_DIVIDER_400KHZ << 8);

            // Enable internal clock (bit2) + keep it on (bit0)
            c1_val |= (1 << 2) | (1 << 0);
            reg_control1.write(c1_val);
            println!("SDHCI: after int clk=0x{:X}", reg_control1.read());

            // Wait for internal clock stable
            count = 0;
            while (reg_control1.read() & (1 << 1)) == 0 {
                if count > timeout {
                    println!("SDHCI: CONTROL1=0x{:X}", reg_control1.read());
                    return Err("Internal clock never stabilized");
                }
                count += 1;
            }
            println!("SDHCI: internal clock stable, CONTROL1=0x{:X}", reg_control1.read());

            // enable SD clock (bit16)
            c1_val = reg_control1.read();
            c1_val |= 1 << 16;
            reg_control1.write(c1_val);
            println!("SDHCI: SD clock enabled, CONTROL1=0x{:X}", reg_control1.read());
        }

        println!("SDHCI: reset() done, bus=3.3V, clock ~400kHz");
        Ok(())
    }

    /// Resets only the CMD line
    fn reset_cmd_line(&mut self) -> Result<(), &'static str> {
        let reg_control1 = self.reg(CONTROL1);
        let val = unsafe { reg_control1.read() };
        unsafe { reg_control1.write(val | C1_SRST_CMD) };

        let mut count = 0;
        let timeout = 100_000;
        while unsafe { reg_control1.read() } & C1_SRST_CMD != 0 {
            if count > timeout {
                println!("SDHCI: partial reset CMD line timed out, CONTROL1=0x{:X}", unsafe {reg_control1.read()});
                return Err("SRST_CMD timed out");
            }
            count += 1;
        }
        println!("SDHCI: partial reset CMD line success");
        Ok(())
    }

    /// Resets only the DAT line
    fn reset_dat_line(&mut self) -> Result<(), &'static str> {
        let reg_control1 = self.reg(CONTROL1);
        let val = unsafe { reg_control1.read() };
        unsafe { reg_control1.write(val | C1_SRST_DATA) };

        let mut count = 0;
        let timeout = 100_000;
        while unsafe { reg_control1.read() } & C1_SRST_DATA != 0 {
            if count > timeout {
                println!("SDHCI: partial reset DAT line timed out, CONTROL1=0x{:X}", unsafe {reg_control1.read()});
                return Err("SRST_DATA timed out");
            }
            count += 1;
        }
        println!("SDHCI: partial reset DAT line success");
        Ok(())
    }

    pub fn send_command(&mut self, cmd: SdCommand, arg: u32) -> Result<(), &'static str> {
        let reg_status = self.reg(STATUS);
        let reg_cmdtm  = self.reg(CMDTM);
        let reg_arg1   = self.reg(ARG1);

        unsafe {
            let timeout = 100_000;
            let mut count = 0;
            while reg_status.read() & (1 << 0) != 0 {
                if count > timeout {
                    println!("SDHCI: command inhibit stuck, STATUS=0x{:X}", reg_status.read());
                    return Err("Command timeout");
                }
                count += 1;
            }
            reg_arg1.write(arg);
            reg_cmdtm.write((cmd as u32) | (1 << 31));

            count = 0;
            while (reg_status.read() & (1 << 0)) != 0 {
                if count > timeout {
                    println!("SDHCI: response inhibit stuck, STATUS=0x{:X}", reg_status.read());
                    return Err("Response timeout");
                }
                count += 1;
            }
        }
        println!("SDHCI: Sent command {:?} with arg {:#010X}", cmd, arg);
        Ok(())
    }

    pub fn read_block(&mut self, block_num: u32, buffer: &mut [u8; 512]) -> Result<(), &'static str> {
        self.send_command(SdCommand::Cmd16, 512)?;
        self.send_command(SdCommand::Cmd17, block_num)?;

        let reg_data = self.reg(DATA);
        unsafe {
            for i in 0..128 {
                let word = reg_data.read();
                buffer[i * 4] = (word & 0xFF) as u8;
                buffer[i * 4 + 1] = ((word >> 8) & 0xFF) as u8;
                buffer[i * 4 + 2] = ((word >> 16) & 0xFF) as u8;
                buffer[i * 4 + 3] = ((word >> 24) & 0xFF) as u8;
            }
        }
        println!("SDHCI: Read block {} successfully", block_num);
        Ok(())
    }

    pub fn handle_interrupts(&mut self) {
        let reg_int_status = self.reg(INTERRUPT);
        let status = unsafe { reg_int_status.read() };
        if status != 0 {
            println!("SDHCI: Interrupt detected {:#010X}", status);
            unsafe {
                self.reg(INTERRUPT).write(status);
            }
        }
    }

    pub fn read_cid(&mut self) -> Result<[u32; 4], &'static str> {
        self.send_command(SdCommand::Cmd2, 0)?;
        Ok([
            unsafe { self.reg(RESP0).read() },
            unsafe { self.reg(RESP1).read() },
            unsafe { self.reg(RESP2).read() },
            unsafe { self.reg(RESP3).read() },
        ])
    }

    pub fn get_rca(&mut self) -> Result<u32, &'static str> {
        self.send_command(SdCommand::Cmd3, 0)?;
        let val = unsafe { self.reg(RESP0).read() };
        Ok(val >> 16)
    }

    pub fn read_csd(&mut self, rca: u32) -> Result<[u32; 4], &'static str> {
        self.send_command(SdCommand::Cmd9, rca << 16)?;
        Ok([
            unsafe { self.reg(RESP0).read() },
            unsafe { self.reg(RESP1).read() },
            unsafe { self.reg(RESP2).read() },
            unsafe { self.reg(RESP3).read() },
        ])
    }

    pub fn get_card_capacity(&mut self, rca: u32) -> u64 {
        let csd = self.read_csd(rca).unwrap_or([0;4]); 
        let c_size = ((csd[1] & 0x3FF) << 10) | ((csd[2] >> 22) & 0x3FF);
        (c_size as u64 + 1) * 512 * 1024
    }

    pub fn detect_card_type(&mut self, rca: u32) -> &'static str {
        let csd = match self.read_csd(rca) {
            Ok(v) => v,
            Err(_) => {
                println!("SDHCI: Could not read CSD for detection, defaulting to unknown");
                return "Unknown";
            }
        };
        let csd_structure = (csd[3] >> 30) & 0b11;

        if csd_structure == 0 {
            println!("SDHCI: Standard Capacity SD Card (SDSC) detected.");
            return "SDSC";
        }

        let c_size = ((csd[1] & 0x3FF) << 10) | ((csd[2] >> 22) & 0x3FF);
        if c_size <= 0xFFF {
            println!("SDHCI: SDHC card detected.");
            "SDHC"
        } else {
            println!("SDHCI: SDXC card detected.");
            "SDXC"
        }
    }
}

unsafe impl Send for bcm2711_sdhci_driver {}
unsafe impl Sync for bcm2711_sdhci_driver {}