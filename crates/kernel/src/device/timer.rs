use crate::sync::Volatile;

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

pub struct bcm2836_l1_intc_driver {
    base: *mut u32,
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
        }
    }

    pub fn timer_reload(&mut self) {
        let local_timer_reload = Volatile(self.base.wrapping_byte_add(REG_TIMER_RELOAD));
        let clear_reload = CR_REG_CLEAR | CR_REG_RELOAD;
        unsafe {
            local_timer_reload.write(clear_reload);
        }
    }

    pub fn timer_clear(&mut self) {
        let local_timer_csr = Volatile(self.base.wrapping_byte_add(REG_TIMER_CSR));
        let new_csr = 0;
        unsafe { local_timer_csr.write(new_csr) };

        let local_timer_reload = Volatile(self.base.wrapping_byte_add(REG_TIMER_RELOAD));
        let clear_reload = CR_REG_CLEAR;
        unsafe {
            local_timer_reload.write(clear_reload);
        }
    }

    pub fn irq_source(&self) -> u32 {
        // TODO: Require that interrupts are disabled here?
        let current_core = crate::core_id() & 0b11;
        let pending_interrupt = Volatile(
            self.base
                .wrapping_byte_add(REG_PENDING_BASE)
                .wrapping_add(current_core as usize),
        );

        unsafe { pending_interrupt.read() }
    }

    pub fn start_timer(&mut self, core: usize, ns: u64) {
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

        let core0_mode = CNTPNS_IRQ;
        unsafe {
            local_timer_cores.add(core).write_volatile(core0_mode);
        }

        let clear_reload = CR_REG_CLEAR | CR_REG_RELOAD;
        unsafe {
            local_timer_reload.write(clear_reload);
        }
    }
}

unsafe impl Send for bcm2836_l1_intc_driver {}
