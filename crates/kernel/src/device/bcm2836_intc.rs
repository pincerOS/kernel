#![allow(dead_code)]

// TODO: handle the global interrupt controller on pi3?
// interrupt-controller@7e00b200
//   compatible: ["brcm,bcm2836-armctrl-ic"]
//   reg: mapped { addr = 0x3f00b200, size = 0x00000200 }
//   interrupt-controller:
//   #interrupt-cells: 0x000002
//   interrupt-parent: 0x000018
//   interrupts: 0x00000800000004
//   phandle: <1>

use crate::event::context::Context;
use crate::sync::{HandlerTableInner, UnsafeInit, Volatile};

const ROUTING_CORE_MASK: u32 = 0b0111;
const ROUTING_IRQ: u32 = 0b0000;
const ROUTING_FIQ: u32 = 0b0100;

const CSR_RELOAD_MASK: u32 = 0x0FFFFFFF;
const CSR_TIMER_ENABLE: u32 = 0x10000000;
const CSR_INT_ENABLE: u32 = 0x20000000;
const CSR_INT_PENDING: u32 = 0x80000000;

const CR_REG_CLEAR: u32 = 0x40000000;
const CR_REG_RELOAD: u32 = 0x80000000;

// PS: "secure physical", PNS: "non-secure physical", HP: "hypervisor physical", V: "virtual physical"
// The correct one for EL1 is PNS?
const CNTPS_IRQ: u32 = 0b0000_0001;
const CNTPNS_IRQ: u32 = 0b0000_0010;
const CNTHP_IRQ: u32 = 0b0000_0100;
const CNTV_IRQ: u32 = 0b0000_1000;
const CNTPS_FIQ: u32 = 0b0001_0000;
const CNTPNS_FIQ: u32 = 0b0010_0000;
const CNTHP_FIQ: u32 = 0b0100_0000;
const CNTV_FIQ: u32 = 0b1000_0000;

const REG_TIMER_ROUTING: usize = 0x24;
const REG_TIMER_CSR: usize = 0x34;
const REG_TIMER_RELOAD: usize = 0x38;
const REG_TIMER_CORES: usize = 0x40;
const REG_PENDING_BASE: usize = 0x60;

pub static LOCAL_INTC: UnsafeInit<bcm2836_l1_intc_driver> = unsafe { UnsafeInit::uninit() };

#[allow(nonstandard_style)]
pub struct bcm2836_l1_intc_driver {
    base: *mut u32,
    pub isr_table: IsrTable,
}

const IRQ_COUNT: usize = 32;

type Isr = fn(&mut Context);

pub struct IsrTable(HandlerTableInner<IRQ_COUNT>);

impl IsrTable {
    pub fn new(fallback: fn(&mut Context)) -> Self {
        Self(HandlerTableInner::new(fallback as usize))
    }
    pub fn get(&self, num: usize) -> fn(&mut Context) {
        unsafe { core::mem::transmute::<usize, _>(self.0.get(num)) }
    }
    pub fn set(&self, num: usize, func: fn(&mut Context)) {
        self.0.set(num, func as usize);
    }
}

bitflags::bitflags! {
    #[derive(Debug)]
    pub struct IRQSource: u32 {
        const LOCAL_TIMER = 1 << 11;
        const AXI_OUTSTANDING = 1 << 10;
        const PMU = 1 << 9;
        const GPU = 1 << 8;
        const MAILBOX_3 = 1 << 7;
        const MAILBOX_2 = 1 << 6;
        const MAILBOX_1 = 1 << 5;
        const MAILBOX_0 = 1 << 4;
        const CNTVIRQ = 1 << 3;
        const CNTHPIRQ = 1 << 2;
        const CNTPNSIRQ = 1 << 1;
        const CNTPSIRQ = 1 << 0;
    }
}

pub const IRQ_LOCAL_TIMER: usize = 11;
pub const IRQ_AXI_OUTSTANDING: usize = 10;
pub const IRQ_PMU: usize = 9;
pub const IRQ_GPU: usize = 8;
pub const IRQ_MAILBOX_3: usize = 7;
pub const IRQ_MAILBOX_2: usize = 6;
pub const IRQ_MAILBOX_1: usize = 5;
pub const IRQ_MAILBOX_0: usize = 4;
pub const IRQ_CNTVIRQ: usize = 3;
pub const IRQ_CNTHPIRQ: usize = 2;
pub const IRQ_CNTPNSIRQ: usize = 1;
pub const IRQ_CNTPSIRQ: usize = 0;

fn irq_not_handled(_ctx: &mut Context) {
    panic!("IRQ not handled");
}

// TODO: reference https://s-matyukevich.github.io/raspberry-pi-os/docs/lesson03/linux/interrupt_controllers.html
impl bcm2836_l1_intc_driver {
    // local_intc@40000000
    //   compatible: brcm,bcm2836-l1-intc\x00
    //   reg: { addr = 0x40000000, size = 0x00000100 }
    //   interrupt-controller:
    //   #interrupt-cells: \x00\x00\x00\x02
    //   interrupt-parent: \x00\x00\x00\x18
    //   phandle: <24>
    pub fn new(base_addr: *mut ()) -> Self {
        Self {
            base: base_addr.cast(),
            isr_table: IsrTable::new(irq_not_handled),
        }
    }

    pub fn timer_reload(&self) {
        let local_timer_reload = Volatile(self.base.wrapping_byte_add(REG_TIMER_RELOAD));
        let clear_reload = CR_REG_CLEAR | CR_REG_RELOAD;
        unsafe { local_timer_reload.write(clear_reload) };
    }

    pub fn timer_clear(&self) {
        let local_timer_csr = Volatile(self.base.wrapping_byte_add(REG_TIMER_CSR));
        let new_csr = 0;
        unsafe { local_timer_csr.write(new_csr) };

        let local_timer_reload = Volatile(self.base.wrapping_byte_add(REG_TIMER_RELOAD));
        let clear_reload = CR_REG_CLEAR;
        unsafe { local_timer_reload.write(clear_reload) };
    }

    /// Reads the pending interrupt register for a core (0-3)
    ///
    /// Safety: ???
    pub unsafe fn irq_source(&self, core: u32) -> IRQSource {
        assert!(core < 4);
        let pending_interrupt = Volatile(
            self.base
                .wrapping_byte_add(REG_PENDING_BASE)
                .wrapping_add(core as usize),
        );
        let source = unsafe { pending_interrupt.read() };
        IRQSource::from_bits_retain(source)
    }

    pub fn start_timer(&self, core: usize, ns: u64) {
        let local_timer_routing = Volatile(self.base.wrapping_byte_add(REG_TIMER_ROUTING));
        let local_timer_csr = Volatile(self.base.wrapping_byte_add(REG_TIMER_CSR));
        let local_timer_reload = Volatile(self.base.wrapping_byte_add(REG_TIMER_RELOAD));
        let local_timer_cores = self.base.wrapping_byte_add(REG_TIMER_CORES);

        let routing = ROUTING_IRQ | (ROUTING_CORE_MASK & core as u32);
        unsafe { local_timer_routing.write(routing) };

        // TODO: get actual timer frequency
        let timer_period = ((ns * 10u64) / 261) as u32;
        let new_csr = (CSR_RELOAD_MASK & timer_period) | CSR_TIMER_ENABLE | CSR_INT_ENABLE;
        unsafe { local_timer_csr.write(new_csr) };

        let core_mode = CNTPNS_IRQ;
        unsafe { local_timer_cores.add(core).write_volatile(core_mode) };

        let clear_reload = CR_REG_CLEAR | CR_REG_RELOAD;
        unsafe { local_timer_reload.write(clear_reload) };
    }

    pub fn enable_irq_cntpnsirq(&self, core: usize) {
        let local_timer_cores = self.base.wrapping_byte_add(REG_TIMER_CORES);
        let core_mode = CNTPNS_IRQ;
        unsafe { local_timer_cores.add(core).write_volatile(core_mode) };
    }
}

fn local_timer_handler(ctx: &mut Context) {
    let irq = LOCAL_INTC.get();
    irq.timer_reload();
    unsafe { crate::event::timer_handler(ctx) };
}

unsafe impl Sync for bcm2836_l1_intc_driver {}
unsafe impl Send for bcm2836_l1_intc_driver {}

#[no_mangle]
pub unsafe extern "C" fn exception_handler_irq(
    ctx: &mut Context,
    _elr: u64,
    _spsr: u64,
    _esr: u64,
    _arg: u64,
) -> *mut Context {
    let irq = LOCAL_INTC.get();
    let core = crate::arch::core_id() & 0b11;
    let source = unsafe { irq.irq_source(core) };

    // println!("irq {source:?} on core {core}");

    let mut bits = source.bits();
    let mut idx = 0;
    while bits != 0 {
        let shift = bits.trailing_zeros();

        let irq_num = (idx + shift) as usize;
        let handler = irq.isr_table.get(irq_num);
        handler(ctx);

        bits >>= shift + 1;
        idx += shift + 1;
    }

    ctx
}
