use core::arch::asm;

fn get_time_ticks() -> usize {
    let time;
    unsafe { asm!("mrs {time}, cntpct_el0", time = out(reg) time) };
    time
}
fn get_freq_ticks() -> usize {
    let freq;
    unsafe { asm!("mrs {freq}, cntfrq_el0", freq = out(reg) freq) };
    freq
}
fn convert_time_to_ticks(μs: usize) -> usize {
    (get_freq_ticks() / 250_000) * μs / 4
}
fn convert_ticks_to_time(ticks: usize) -> usize {
    // TODO: reduce chances of overflow
    (ticks * 1_000_000) / get_freq_ticks()
}

pub fn get_time() -> usize {
    convert_ticks_to_time(get_time_ticks())
}

pub fn spin_sleep(μs: usize) {
    let target = get_time() + μs;
    spin_sleep_until(target)
}

pub fn spin_sleep_until(target: usize) {
    let target = convert_time_to_ticks(target);
    while get_time_ticks() < target {
        // TODO: yield vs wfe/wfi?
        unsafe {
            asm!("yield");
        }
    }
}
