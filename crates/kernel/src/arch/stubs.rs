pub fn get_time_ticks() -> usize {
    unimplemented!()
}
pub fn get_freq_ticks() -> usize {
    unimplemented!()
}
pub fn halt() -> ! {
    loop {}
}
pub fn core_id() -> u32 {
    unimplemented!()
}

pub unsafe fn wfe() {}
pub unsafe fn sev() {}
pub unsafe fn yield_() {}
pub unsafe fn udf() -> ! {
    loop {}
}

pub fn debug_get_sp() -> usize {
    0
}

pub mod boot {
    #[allow(unused)]
    extern "C" {
        pub fn kernel_entry();
        pub fn kernel_entry_alt();
    }

    const STACK_SIZE_LOG2: usize = 16;
    const STACK_SIZE: usize = 1 << STACK_SIZE_LOG2;

    #[no_mangle]
    #[link_section = ".bss"]
    pub static mut STACKS: [[u128; STACK_SIZE / 16]; 4] = [[0u128; STACK_SIZE / 16]; 4];
}

pub mod interrupts {
    #[derive(Debug)]
    pub struct InterruptsState(());

    pub fn get_interrupts() -> InterruptsState {
        InterruptsState(())
    }
    pub unsafe fn disable_interrupts() -> InterruptsState {
        InterruptsState(())
    }
    pub unsafe fn enable_interrupts() -> InterruptsState {
        InterruptsState(())
    }
    pub unsafe fn restore_interrupts(_state: InterruptsState) {}
}
