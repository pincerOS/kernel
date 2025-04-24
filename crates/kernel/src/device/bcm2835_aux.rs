use crate::sync::{InterruptSpinLock, UnsafeInit, Volatile};

use super::gpio::{GpioFunction, GpioPull};
use super::GPIO;

// https://www.raspberrypi.org/app/uploads/2012/02/BCM2835-ARM-Peripherals.pdf

// TODO: This shouldn't actually use an interrupt-disabling lock, but we
// don't currently have a better logging system.  (Interrupt handlers
// should not print, and can still deadlock with this, but it should
// make those deadlocks rare.)
pub type MiniUartLock = InterruptSpinLock<MiniUart>;

pub static MINI_UART: UnsafeInit<MiniUartLock> = unsafe { UnsafeInit::uninit() };

pub struct MiniUart {
    base: *mut (),
}

#[allow(dead_code)]
#[rustfmt::skip]
impl MiniUart {
    const AUX_IRQ: usize = 0x0000;            // Auxiliary Interrupt status 3
    const AUX_ENABLES: usize = 0x0004;        // Auxiliary enables 3
    const AUX_MU_IO_REG: usize = 0x0040;      // Mini Uart I/O Data 8
    const AUX_MU_IER_REG: usize = 0x0044;     // Mini Uart Interrupt Enable 8
    const AUX_MU_IIR_REG: usize = 0x0048;     // Mini Uart Interrupt Identify 8
    const AUX_MU_LCR_REG: usize = 0x004C;     // Mini Uart Line Control 8
    const AUX_MU_MCR_REG: usize = 0x0050;     // Mini Uart Modem Control 8
    const AUX_MU_LSR_REG: usize = 0x0054;     // Mini Uart Line Status 8
    const AUX_MU_MSR_REG: usize = 0x0058;     // Mini Uart Modem Status 8
    const AUX_MU_SCRATCH: usize = 0x005C;     // Mini Uart Scratch 8
    const AUX_MU_CNTL_REG: usize = 0x0060;    // Mini Uart Extra Control 8
    const AUX_MU_STAT_REG: usize = 0x0064;    // Mini Uart Extra Status 32
    const AUX_MU_BAUD_REG: usize = 0x0068;    // Mini Uart Baudrate 16
    const AUX_SPI0_CNTL0_REG: usize = 0x0080; // SPI 1 Control register 0 32
    const AUX_SPI0_CNTL1_REG: usize = 0x0084; // SPI 1 Control register 1 8
    const AUX_SPI0_STAT_REG: usize = 0x0088;  // SPI 1 Status 32
    const AUX_SPI0_IO_REG: usize = 0x0090;    // SPI 1 Data 32
    const AUX_SPI0_PEEK_REG: usize = 0x0094;  // SPI 1 Peek 16
    const AUX_SPI1_CNTL0_REG: usize = 0x00C0; // SPI 2 Control register 0 32
    const AUX_SPI1_CNTL1_REG: usize = 0x00C4; // SPI 2 Control register 1 8
    const AUX_SPI1_STAT_REG: usize = 0x00C8;  // SPI 2 Status 32
    const AUX_SPI1_IO_REG: usize = 0x00D0;    // SPI 2 Data 32
    const AUX_SPI1_PEEK_REG: usize = 0x00D4;  // SPI 2 Peek 16
}

bitflags::bitflags! {
    struct IirReg: u32 {
        // Access to the MS 8 bits of the 16-bit baudrate register.
        // (Only if DLAB_ACCESS bit is set in LCR)
        const BAUD_MSBYTE = 0xFF;

        const RECV_INT_ENABLE = 1 << 1;
        const TRANS_INT_ENABLE = 1 << 0;
    }

    struct IerReg: u32 {
        const FIFO_ENABLES = 0b11 << 6;
        const INT_ID_MASK = 0b11 << 1;
        const CLR_RECV_FIFO = 1 << 2;
        const CLR_TRANS_FIFO = 1 << 1;
        const INT_PENDING = 1 << 0;
    }

    struct LcrReg: u32 {
        const DLAB_ACCESS = 1 << 7;
        const BREAK = 1 << 6;
        // Bit 1 is undocumented, but must be 1 for 8 bit mode.
        const DATA_8BIT = 0b11 << 0;
    }

    struct McrReg: u32 {
        const RTS = 1 << 1;
    }

    struct LsrReg: u32 {
        const TRANS_IDLE = 1 << 6;
        const TRANS_READY = 1 << 5;
        const RECV_OVER = 1 << 1;
        const DATA_READY = 1 << 0;
    }

    struct MsrReg: u32 {
        const CTS_STATUS = 1 << 5;
    }

    struct CntrlReg: u32 {
        const CTS_ASSERT_LOW = 1 << 7;
        const RTS_ASSERT_LOW = 1 << 6;
        const RTS_AUTO_MASK = 0b11 << 4;
        const TRANS_AUTO = 1 << 3;
        const RECV_AUTO = 1 << 2;
        const TRANS_ENABLE = 1 << 1;
        const RECV_ENABLE = 1 << 0;
    }

    struct StatReg: u32 {
        const TRANS_LEVEL = 0b1111 << 24;
        const RECV_LEVEL = 0b1111 << 16;
        const TRANS_DONE = 1 << 9;
        const TRANS_EMPTY = 1 << 8;
        const CTS_STATUS = 1 << 7;
        const RTS_STATUS = 1 << 6;
        const TRANS_FULL = 1 << 5;
        const RECV_OVER = 1 << 4;
        const TRANS_IDLE = 1 << 3;
        const RECV_IDEL = 1 << 2;
        const TRANS_AVAIL = 1 << 1;
        const RECV_AVAIL = 1 << 1;
    }
}

impl MiniUart {
    pub unsafe fn new(base: *mut ()) -> Self {
        // TODO[hardware]: proper mini UART initialization
        // (this is enough to convince qemu that it's initialized)
        let this = Self { base };
        unsafe {
            // Enable mini UART, disable both SPI modules
            this.reg(Self::AUX_ENABLES).write(0b001);

            this.reg(Self::AUX_MU_IER_REG).write(IerReg::empty().bits());
            this.reg(Self::AUX_MU_CNTL_REG)
                .write(CntrlReg::empty().bits());
            this.reg(Self::AUX_MU_LCR_REG)
                .write((LcrReg::DATA_8BIT).bits());
            this.reg(Self::AUX_MU_MCR_REG).write(McrReg::empty().bits());
            this.reg(Self::AUX_MU_IER_REG).write(IerReg::empty().bits()); // ????
            this.reg(Self::AUX_MU_IIR_REG).write(0xC6); // ???

            let clock_rate;
            {
                let mut mailbox = super::MAILBOX.get().lock();
                clock_rate = mailbox
                    .get_property(super::mailbox::PropGetClockRate {
                        id: super::mailbox::CLOCK_CORE,
                    })
                    .unwrap()
                    .rate;
            }

            // baud_rate = sys_clock_freq / (8 * (baud_reg + 1))
            // baud_reg = sys_clock_freq / (baud_rate * 8) - 1
            let target_baud_rate = 115200;
            let sys_clock_freq = clock_rate;
            let baud_reg = sys_clock_freq / (target_baud_rate * 8) - 1;

            this.reg(Self::AUX_MU_BAUD_REG).write(baud_reg);

            {
                let mut gpio = GPIO.get().lock();
                gpio.set_function(14, GpioFunction::Alt5);
                gpio.set_function(15, GpioFunction::Alt5);
                gpio.set_pull(14, GpioPull::None);
                gpio.set_pull(15, GpioPull::None);
            }

            this.reg(Self::AUX_MU_CNTL_REG)
                .write((CntrlReg::RECV_ENABLE | CntrlReg::TRANS_ENABLE).bits());
        }
        this
    }

    fn reg(&self, reg: usize) -> Volatile<u32> {
        Volatile(self.base.wrapping_byte_add(reg).cast::<u32>())
    }

    unsafe fn read_lsr(&mut self) -> LsrReg {
        LsrReg::from_bits_retain(unsafe { self.reg(Self::AUX_MU_LSR_REG).read() })
    }

    pub fn writec(&mut self, c: u8) {
        unsafe {
            loop {
                let lsr = self.read_lsr();
                if lsr.contains(LsrReg::RECV_OVER) {
                    // !!!
                }
                if lsr.contains(LsrReg::TRANS_READY) {
                    break;
                }
            }
            self.reg(Self::AUX_MU_IO_REG).write(c as u32);
        }
    }
    pub fn getc(&mut self) -> u8 {
        unsafe {
            loop {
                let lsr = self.read_lsr();
                if lsr.contains(LsrReg::RECV_OVER) {
                    // !!!
                }
                if lsr.contains(LsrReg::DATA_READY) {
                    break;
                }
            }
            self.reg(Self::AUX_MU_IO_REG).read() as u8
        }
    }
    pub fn try_getc(&mut self) -> Option<u8> {
        unsafe {
            let lsr = self.read_lsr();
            if lsr.contains(LsrReg::RECV_OVER) {
                // !!!
            }
            if lsr.contains(LsrReg::DATA_READY) {
                Some(self.reg(Self::AUX_MU_IO_REG).read() as u8)
            } else {
                None
            }
        }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.writec(*b);
        }
        // if super::CONSOLE.is_initialized() {
        //     let mut console = super::CONSOLE.get().lock();
        //     console.input(bytes);
        //     if bytes.contains(&b'\n') {
        //         console.render();
        //     }
        //     drop(console);
        //     crate::sync::spin_sleep(3000);
        // }
    }
}

unsafe impl Send for MiniUart {}

impl core::fmt::Write for MiniUart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_bytes(s.as_bytes());
        Ok(())
    }
}

impl core::fmt::Write for &MiniUartLock {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut inner = self.lock();
        inner.write_bytes(s.as_bytes());
        Ok(())
    }
    fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> core::fmt::Result {
        let mut inner = self.lock();
        inner.write_fmt(args)
    }
}
