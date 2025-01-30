pub struct InterruptsState(u64);

impl core::fmt::Debug for InterruptsState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("InterruptsState")
            .field(&format_args!("{:#08x}", self.0))
            .finish()
    }
}

extern "C" {
    fn get_interrupts_asm() -> u64;
    fn set_interrupts_asm(state: u64);
    fn disable_interrupts_asm();
    fn enable_interrupts_asm();
}

core::arch::global_asm!(
    "get_interrupts_asm: mrs x0, DAIF; ret",
    "set_interrupts_asm: msr DAIF, x0; ret",
    "disable_interrupts_asm: msr DAIFSet, #0b1111; ret",
    "enable_interrupts_asm: msr DAIFClr, #0b1111; ret",
);

pub fn get_interrupts() -> InterruptsState {
    InterruptsState(unsafe { get_interrupts_asm() })
}
pub unsafe fn disable_interrupts() -> InterruptsState {
    let state = get_interrupts();
    unsafe { disable_interrupts_asm() };
    state
}
pub unsafe fn enable_interrupts() -> InterruptsState {
    let state = get_interrupts();
    unsafe { enable_interrupts_asm() };
    state
}
pub unsafe fn restore_interrupts(state: InterruptsState) {
    unsafe { set_interrupts_asm(state.0) };
}
