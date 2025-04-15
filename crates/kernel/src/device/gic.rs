#![allow(dead_code)]

//! GIC-400 (Generic Interrupt Controller) driver.
//!
//! Based on ARM GIC-400 reference manual (https://developer.arm.com/documentation/ddi0471/b)
//! and ARM Generic Interrupt Controller Architecture Specification (https://developer.arm.com/documentation/ihi0048/b)
//! and BCM2711 ARM Peripherals (https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf)
//! https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/drivers/irqchip/irq-gic.c:
//!
//! Currently not implementing hypervisor and virtual interrupts
//!
//! gic_init must run on the primary core before any other core
//! gic_init_other_cores must run on all other cores (excluding the primary core)
//!
//! To register an ISR for an interrupt:
//!     Use gic_register_isr(irq: usize, isr: fn()) for simple cases
//!     Use gic_register_isr_detailed(irq: usize, isr: fn(), target_cpus: u8, int_type: bool, priority: u8) for more complex cases
//!         Additional functions gic_set_irq_priority, gic_set_irq_affinity, gic_set_irq_type are available for more control
//!
//! To unregister an ISR for an interrupt: Use gic_unregister_isr(irq: usize)
//!
//! Finally, gic_irq_handler() is the main IRQ handler
//!
//! To send a software generated interrupt: gic_send_sgi(target_list_filter: u8, target_cpus: u8, irq: u8)
//! To set a priority mask (normally GIC handles this): run gic_set_interrupt_priority_mask(priority: u8) -> u32
//!     and gic_reset_interrupt_priority_mask(old_pmr: u32) to reset it back
//!
//! IMPORTANT!!!:
//! If you have a level-triggered interrupt, you must clear it when doen with gic_clear_pending_irq(irq: usize)
//!

//TODO: Handling software generated interrupts? 0-15 (make core specific?)
//TODO: Handling private peripheral interrupts? 16-31 (make core specific?)
//TODO: Handling FIQs?

use core::arch::asm;

use crate::event::context::Context;
use crate::sync::{HandlerTableInner, SpinLockInner, UnsafeInit};

const SPI_COUNT: usize = 192;
const IRQ_COUNT: usize = SPI_COUNT + 32;

const CPU_MASK: u32 = 0x0F;

const GICD_DISABLE: u32 = 0x0;
const GICD_ENABLE: u32 = 0x1;

const GICC_DISABLE: u32 = 0x0;
const GICC_ENABLE: u32 = 0x1;

//The device tree makes the base address 0x1000 too high, therefore we need to subtract it
const GIC_UNOFFSET: usize = 0x1000;

//https://developer.arm.com/documentation/ihi0048/b/GIC-Partitioning/The-Distributor/Interrupt-IDs
//Reference for GIC Distributor (https://developer.arm.com/documentation/ddi0471/b/programmers-model/distributor-register-summary?lang=en)
const GICD_OFFSET: usize = 0x1000 - GIC_UNOFFSET;
const GICD_CTLR: usize = 0x0000; // Distributor Control Register (0x000)
const GICD_TYPER: usize = 0x0004; // Interrupt Controller Type Register (0x004)
const GICD_IIDR: usize = 0x0008; // Distributor Implementer Identification Register (0x008)
const GICD_IGROUP: usize = 0x0080; // Interrupt Group Registers (0x080-0x0BC)
const GICD_ISENABLER: usize = 0x0100; // Interrupt Set-Enable Registers (0x100, 0x104-0x13C)
const GICD_ICENABLER: usize = 0x0180; // Interrupt Clear-Enable Registers (0x180, 0x184-0x1BC)
const GICD_ISPENDR: usize = 0x0200; // Interrupt Set-Pending Registers (0x200-0x23C)
const GICD_ICPENDR: usize = 0x0280; // Interrupt Clear-Pending Registers (0x280-0x2BC)
const GICD_ISACTIVER: usize = 0x0300; // Interrupt Set-Active Registers (0x300-0x33C)
const GICD_ICACTIVER: usize = 0x0380; // Interrupt Clear-Active Registers (0x380-0x3BC)
const GICD_IPRIORITYR: usize = 0x0400; // Interrupt Priority Registers (0x400-0x5FC)
const GICD_ITARGETSR: usize = 0x0800; // Interrupt Processor Targets Registers (0x800-0x81C, 0x820-0x9FC)
const GICD_ICFGR: usize = 0x0C00; // Interrupt Configuration Registers (0xC00, 0xC04, 0xC08-0xC7C)
const GICD_PPISR: usize = 0x0D00; // Private Peripheral Interrupt Status Register, GICD_PPISR (0xD00)
const GICD_SPISR: usize = 0x0D04; // Shared Peripheral Interrupt Status Registers, GICD_SPISRn (0xD04-0xD3C)
const GICD_SGIR: usize = 0x0F00; // Software Generated Interrupt Register (0xF00)
const GICD_CPENDSGIR: usize = 0x0F10; // SGI Clear-Pending Registers (0xF10-0xF1C)
const GICD_SPENDSGIR: usize = 0x0F20; // SGI Set-Pending Registers (0xF20-0xF2C)
const GICD_PIDR4: usize = 0x0FD0; // Peripheral ID4 Register (0xFD0)
const GICD_PIDR5: usize = 0x0FD4; // Peripheral ID5 Register (0xFD4)
const GICD_PIDR6: usize = 0x0FD8; // Peripheral ID6 Register (0xFD8)
const GICD_PIDR7: usize = 0x0FDC; // Peripheral ID7 Register (0xFDC)
const GICD_PIDR0: usize = 0x0FE0; // Peripheral ID0 Register (0xFE0)
const GICD_PIDR1: usize = 0x0FE4; // Peripheral ID1 Register (0xFE4)
const GICD_PIDR2: usize = 0x0FE8; // Peripheral ID2 Register (0xFE8)
const GICD_PIDR3: usize = 0x0FEC; // Peripheral ID3 Register (0xFEC)
const GICD_CIDR0: usize = 0x0FF0; // Component ID0 Register (0xFF0)
const GICD_CIDR1: usize = 0x0FF4; // Component ID1 Register (0xFF4)
const GICD_CIDR2: usize = 0x0FF8; // Component ID2 Register (0xFF8)
const GICD_CIDR3: usize = 0x0FFC; // Component ID3 Register (0xFFC)

//Reference for GIC CPU Interface (https://developer.arm.com/documentation/ddi0471/b/programmers-model/cpu-interface-register-summary?lang=en)
//https://developer.arm.com/documentation/ihi0048/b/GIC-Partitioning/CPU-interfaces
const GICC_OFFSET: usize = 0x2000 - GIC_UNOFFSET;
const GICC_CTLR: usize = 0x0000; // CPU Interface Control Register (0x000)
const GICC_PMR: usize = 0x0004; // Interrupt Priority Mask Register (0x004)
const GICC_BPR: usize = 0x0008; // Binary Point Register (0x008)
const GICC_IAR: usize = 0x000C; // Interrupt Acknowledge Register (0x00C)
const GICC_EOIR: usize = 0x0010; // End of Interrupt Register (0x010)
const GICC_RPR: usize = 0x0014; // Running Priority Register (0x014)
const GICC_HPPIR: usize = 0x0018; // Highest Priority Pending Interrupt Register (0x018)
const GICC_ABPR: usize = 0x001C; // Aliased Binary Point Register (0x01C)
const GICC_AIAR: usize = 0x0020; // Aliased Interrupt Acknowledge Register (0x020)
const GICC_AEOIR: usize = 0x0024; // Aliased End of Interrupt Register (0x024)
const GICC_AHPPIR: usize = 0x0028; // Aliased Highest Priority Pending Interrupt Register (0x028)
const GICC_APR0: usize = 0x00D0; // Active Priority Register (0x0D0)
const GICC_NSAPR0: usize = 0x00E0; // Non-Secure Active Priority Register (0x0E0)
const GICC_IIDR: usize = 0x00FC; // CPU Interface Identification Register (0x0FC)
const GICC_DIR: usize = 0x1000; // Deactivate Interrupt Register (0x1000)

pub static GIC: UnsafeInit<Gic400Driver> = unsafe { UnsafeInit::uninit() };

fn isb() {
    //Double check if this is correct
    unsafe { asm!("isb", options(nostack, nomem, preserves_flags)) }
}

pub fn irq_not_handled(_ctx: &mut Context) {
    panic!("IRQ not handled");
}

/// Handles the interrupt and asks handler to handle it
/// Level-triggered interrupts must be cleared by associaetd irq handler or else it will be called again
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gic_irq_handler(
    ctx: &mut Context,
    _elr: u64,
    _spsr: u64,
    _esr: u64,
    _arg: u64,
) -> *mut Context {
    let _core = crate::arch::core_id() & 0b11;
    //Loop to handle all batched IRQs

    let gic = GIC.get();
    loop {
        //read IAR for the interrupt number
        let iar = unsafe { gic.reg_read_cpui(GICC_IAR) };
        let irq = iar & 0x3ff;
        // println!("IRQ: {irq} on core {_core}");

        //spuriuos interrupt, ignore
        if irq > 1020 {
            break;
        }

        if irq >= IRQ_COUNT as u32 {
            panic!("Invalid IRQ number");
        }

        //TODO: This may cause a few issues on the USB handling side, may need to push back
        unsafe { gic.reg_write_cpui(GICC_EOIR, iar) };

        //acknowledge the interrupt
        //fence
        //TODO: Switch to dsb?
        isb();

        //May run into issuse of group 0, group1
        //GICC_EOIR is used for processing Group 0 interrupts
        //GICC_AEOIR is used for processing Group 1 interrupts.

        //SGI Handling
        if irq < 16 {
            let _core_from = (iar >> 10) & 0x3;
            //do I need to check anything really?

            //TODO: Indepedent SGI handling or default to default?
        }

        //IRQ actual handling
        // Level-triggered interrupts must be cleared by associated irq
        // handler or else it will be called again
        let irq_handler = gic.isr_table.get(irq as usize);
        irq_handler(ctx);

        //TODO: HAndle level-triggered interrupts?
        // GICC_DIR is not necessary, Security Extensions are not enabled
    }

    ctx
}

type Isr = fn(&mut Context);

pub struct IsrTable(HandlerTableInner<IRQ_COUNT>);

impl IsrTable {
    pub fn new(fallback: Isr) -> Self {
        Self(HandlerTableInner::new(fallback as usize))
    }
    pub fn get(&self, num: usize) -> Isr {
        unsafe { core::mem::transmute::<usize, _>(self.0.get(num)) }
    }
    pub fn set(&self, num: usize, func: Isr) {
        self.0.set(num, func as usize);
    }
}

pub struct Gic400Driver {
    base_addr: *mut u32,

    // TODO: Add support for IRQs specific to a cpu
    isr_table: IsrTable,

    irq_set_lock: SpinLockInner,
    irq_affinity_lock: SpinLockInner,
    irq_priority_lock: SpinLockInner,
}

unsafe impl Sync for Gic400Driver {}

impl Gic400Driver {
    /// Creates and initializes a new GIC-400 driver.
    /// Maps the distributor and cpu_interface registers to the base address
    ///
    /// Initialize the GIC-400 Distributor and CPU interface for primary core
    /// (should be run once) Non-Thread Safe
    pub unsafe fn init(base_addr: *mut ()) -> Self {
        let this = Self {
            base_addr: base_addr.cast(),

            isr_table: IsrTable::new(irq_not_handled),
            irq_set_lock: SpinLockInner::new(),
            irq_affinity_lock: SpinLockInner::new(),
            irq_priority_lock: SpinLockInner::new(),
        };

        this.init_distributor();
        let gicc_type = this.check_cpu_identification();
        println!("| GICC Type: {:#010x}", gicc_type);

        //Add interrupt registering here

        //More care is needed for SGIs and PPIs as they are core specific

        this.init_per_core();

        this
    }

    /// Initlizalize the GIC-400 CPU interface for other cores
    pub fn init_per_core(&self) {
        println!(
            "| GIC initing {} CPU core interface",
            crate::arch::core_id() & 0b11
        );
        self.init_cpu_interface();
    }

    // initialize the GIC-400 distributor (CPU DEPENDENT)
    pub fn init_distributor(&self) {
        unsafe { self.reg_write_dist(GICD_CTLR, GICD_DISABLE) };

        let mut cpumask = CPU_MASK | (CPU_MASK << 8);
        cpumask |= cpumask << 16;

        for i in (32..IRQ_COUNT).step_by(4) {
            unsafe { self.reg_write_dist(GICD_ITARGETSR + i, cpumask) };
        }

        self.dist_config();

        unsafe { self.reg_write_dist(GICD_CTLR, GICD_ENABLE) };

        let gicd_type = self.get_gicd_type();
        println!("| GICD Type: {:#010x}", gicd_type);
    }

    // initialize the GIC-400 cpu interface (CPU DEPENDENT)
    pub fn init_cpu_interface(&self) {
        self.cpu_config();

        unsafe { self.reg_write_cpui(GICC_PMR, 0xf0) };

        //Active Priorities register implementation
        for i in 0..4 {
            unsafe { self.reg_write_cpui(GICC_APR0 + i * 4, 0) };
        }

        //preserves GICC_ENABLE bits (for GICC.EOLmode ns)
        //TODO: Is this necessary? I don't think the GIC-400 on the Raspi supports Security extensions
        let bypass = unsafe { self.reg_read_cpui(GICC_CTLR) } & 0x1e0;
        unsafe { self.reg_write_cpui(GICC_CTLR, bypass | GICC_ENABLE) };
    }

    unsafe fn reg_write_dist(&self, reg: usize, val: u32) {
        unsafe { core::ptr::write_volatile(self.base_addr.byte_add(GICD_OFFSET + reg), val) }
    }
    unsafe fn reg_write_cpui(&self, reg: usize, val: u32) {
        unsafe { core::ptr::write_volatile(self.base_addr.byte_add(GICC_OFFSET + reg), val) }
    }
    unsafe fn reg_read_dist(&self, reg: usize) -> u32 {
        unsafe { core::ptr::read_volatile(self.base_addr.byte_add(GICD_OFFSET + reg)) }
    }
    unsafe fn reg_read_cpui(&self, reg: usize) -> u32 {
        unsafe { core::ptr::read_volatile(self.base_addr.byte_add(GICC_OFFSET + reg)) }
    }

    /// Sends a software generated interrupt to the targetted cpus
    /// TargetListFilter:
    ///     0b00: Forward the interrupt to the CPU interfaces specified in the CPUTargetList field
    ///     0b01: Forward the interrupt to all CPU interfaces except that of the processor that requested the interrupt.
    ///     0b10: Forward the interrupt only to the CPU interface of the processor that requested the interrupt.
    /// CPUTargetList: target list of cpus (one bit per cpu)
    /// irq: 0-15 for SGI
    ///     
    pub fn send_sgi(&self, target_list_filter: u8, target_cpus: u8, irq: u8) {
        //1 << 15 for the NSATT
        let sgi_val = ((target_list_filter as u32) << 24)
            | ((target_cpus as u32) << 16)
            | (1 << 15)
            | (irq as u32);
        unsafe { self.reg_write_dist(GICD_SGIR, sgi_val) };
    }

    /// returns the original priority mask that must be set back after use
    pub fn set_interrupt_priority_mask(&self, priority: u8) -> u32 {
        let old_pmr = unsafe { self.reg_read_cpui(GICC_PMR) };
        unsafe { self.reg_write_cpui(GICC_PMR, priority as u32) };
        old_pmr
    }

    /// takes in the old priority mask and sets it back
    pub fn reset_interrupt_priority_mask(&self, old_pmr: u32) {
        unsafe { self.reg_write_cpui(GICC_PMR, old_pmr) };
    }

    pub fn register_isr(&self, irq: usize, isr: fn(&mut Context)) {
        self.register_isr_detailed(irq, isr, 0xf, false, 0xa0);
    }

    /// Does not reset the ISR back to default after unregistering
    pub fn unregister_isr(&self, irq: usize) {
        assert!(irq < IRQ_COUNT);
        self.disable_irq(irq);
        self.isr_table.set(irq, irq_not_handled);
    }

    pub fn register_isr_detailed(
        &self,
        irq: usize,
        isr: fn(&mut Context),
        target_cpus: u8,
        int_type: bool,
        priority: u8,
    ) {
        assert!(irq < IRQ_COUNT);
        self.isr_table.set(irq, isr);
        self.set_irq_affinity(irq, target_cpus);
        self.set_irq_type(irq, int_type);
        self.set_irq_priority(irq, priority);

        self.enable_irq(irq);
    }

    //Sets the priority of the interrupt, 0 is the highest priority and 255 is the lowest
    pub fn set_irq_priority(&self, irq: usize, priority: u8) {
        self.irq_priority_lock.lock();
        //read the IPRIORITYR register for the interrupt
        let mut priority_reg = unsafe { self.reg_read_dist(GICD_IPRIORITYR + (irq / 4) * 4) };
        let shift = (irq % 4) * 8;
        priority_reg &= !(0xff << shift);
        priority_reg |= (priority as u32) << shift;

        //write back the IPRIORITYR register
        unsafe { self.reg_write_dist(GICD_IPRIORITYR + (irq / 4) * 4, priority_reg) };
        self.irq_priority_lock.unlock();
    }

    //sets the affinity of the interrupt to the target_cpus
    pub fn set_irq_affinity(&self, irq: usize, target_cpus: u8) {
        self.irq_affinity_lock.lock();
        //read the ITARGETSR register for the interrupt
        let mut cpumask = unsafe { self.reg_read_dist(GICD_ITARGETSR + (irq / 4) * 4) };
        let shift = (irq % 4) * 8;
        cpumask &= !(0xff << shift);
        cpumask |= (target_cpus as u32) << shift;

        //write back the ITARGETSR register
        unsafe { self.reg_write_dist(GICD_ITARGETSR + (irq / 4) * 4, cpumask) };
        self.irq_affinity_lock.unlock();
    }

    /// Edge/Level-Triggered Interrupts, 0 is level-triggered, 1 is edge-triggered
    /// irq must be disabled before use otherwise issues may arise
    pub fn set_irq_type(&self, irq: usize, int_type: bool) {
        if irq < 16 {
            panic!("Cannot set type for SGI or PPI");
        }
        self.irq_set_lock.lock();

        //read the ICFGR register for the interrupt
        let mut icfgr = unsafe { self.reg_read_dist(GICD_ICFGR + (irq / 16) * 4) };
        let shift = (irq % 16) * 2;
        icfgr &= !(0x3 << shift);
        //TODO: I think this could be issue
        icfgr |= (int_type as u32) << (shift + 1);

        //write back the ICFGR register
        unsafe { self.reg_write_dist(GICD_ICFGR + (irq / 16) * 4, icfgr) };
        self.irq_set_lock.unlock();
    }

    /// Checks the type of interrupt, 0 is level-triggered, 1 is edge-triggered
    /// TODO: CHEcK if this actually works
    pub fn check_irq_type(&self, irq: usize) -> bool {
        if irq < 16 {
            panic!("Cannot check type for SGI or PPI");
        }

        //read the ICFGR register for the interrupt
        let icfgr = unsafe { self.reg_read_dist(GICD_ICFGR + (irq / 16) * 4) };
        let shift = (irq % 16) * 2;
        (icfgr & (1 << shift)) != 0
    }

    /// Clears the pending state of the corresponding peripheral interrupt
    /// Atomic register
    pub fn clear_pending_irq(&self, irq: usize) {
        //write the ICPENDR register for the interrupt
        unsafe { self.reg_write_dist(GICD_ICPENDR + (irq / 32) * 4, 1 << (irq % 32)) };
    }

    /// Enable the interrupt
    /// Atomic register
    pub fn enable_irq(&self, irq: usize) {
        //write the ISENABLER register for the interrupt
        unsafe { self.reg_write_dist(GICD_ISENABLER + (irq / 32) * 4, 1 << (irq % 32)) };
    }

    /// Deactivate the interrupt
    /// Atomic register
    pub fn disable_irq(&self, irq: usize) {
        //write the ICENABLER register for the interrupt
        unsafe { self.reg_write_dist(GICD_ICENABLER + (irq / 32) * 4, 1 << (irq % 32)) };
    }

    /// Checks is interrupt is enabled
    /// Atomic register
    pub fn check_irq_enabled(&self, irq: usize) -> bool {
        //read the ISENABLER register for the interrupt
        let isenabler = unsafe { self.reg_read_dist(GICD_ICENABLER + (irq / 32) * 4) };
        (isenabler & (1 << (irq % 32))) != 0
    }

    /// Checks if interrupt is active
    pub fn check_irq_active(&self, irq: usize) -> bool {
        //read the ISACTIVER register for the interrupt
        let isactiver = unsafe { self.reg_read_dist(GICD_ISACTIVER + (irq / 32) * 4) };
        (isactiver & (1 << (irq % 32))) != 0
    }

    pub fn get_gicd_type(&self) -> u32 {
        unsafe { self.reg_read_dist(GICD_TYPER) }
    }

    fn dist_config(&self) {
        //Sets interrupts to be level triggered
        for i in (32..IRQ_COUNT).step_by(16) {
            unsafe { self.reg_write_dist(GICD_ICFGR + i / 4, 0) };
        }

        //Set priority
        for i in (32..IRQ_COUNT).step_by(4) {
            unsafe { self.reg_write_dist(GICD_IPRIORITYR + i, repeat_byte(0xa0)) };
        }

        //Deactivate all interrupts except for banked CPU registeres
        for i in (32..IRQ_COUNT).step_by(32) {
            unsafe {
                self.reg_write_dist(GICD_ICENABLER + i / 8, 0xffffffff);
                self.reg_write_dist(GICD_ICACTIVER + i / 8, 0xffffffff);
            }
        }
    }

    fn check_cpu_identification(&self) -> u32 {
        unsafe { self.reg_read_cpui(GICC_IIDR) }
    }

    fn cpu_config(&self) {
        //Deactivate SGIs and PPIs
        unsafe {
            self.reg_write_dist(GICD_ICACTIVER, 0xffffffff);
            self.reg_write_dist(GICD_ICENABLER, 0xffffffff);
        }

        //Set priority on the SGIs and PPIs
        for i in (0..32).step_by(4) {
            unsafe { self.reg_write_dist(GICD_IPRIORITYR + i, repeat_byte(0xa0)) };
        }
    }
}

#[inline]
fn repeat_byte(byte: u8) -> u32 {
    u32::from_ne_bytes([byte; 4])
}
