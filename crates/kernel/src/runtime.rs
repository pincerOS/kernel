#[cfg_attr(not(any(test, doc)), panic_handler)]
#[allow(dead_code)]
#[unsafe(no_mangle)]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    use crate::arch::halt;
    use crate::uart;
    use core::sync::atomic;

    static PANICKING: atomic::AtomicBool = atomic::AtomicBool::new(false);

    if PANICKING.swap(true, atomic::Ordering::Relaxed) {
        halt();
    }

    let (location, line, column) = match info.location() {
        Some(loc) => (loc.file(), loc.line(), loc.column()),
        _ => ("unknown", 0, 0),
    };
    if uart::UART.is_initialized() {
        use core::fmt::Write;
        let uart = uart::UART.get();
        // Bypass the UART lock to print the panic message; this isn't sound,
        // but the UART struct only uses mutability to access to the MMIO,
        // so it shouldn't cause issues (beyond the fact that it's already
        // in a panic.)
        let mut guard = unsafe { uart.force_acquire() };
        writeln!(
            &mut *guard,
            "Kernel panic at '{location}:{line}:{column}'\nmessage: {}",
            info.message()
        )
        .ok();
        // Avoid unlocking the lock on drop
        core::mem::forget(guard);
    }

    // TODO: write error message to a fixed location in memory and reset?
    if crate::device::WATCHDOG.is_initialized() {
        // crate::device::LED_OUT.get().put(0b00011000);
        // Shut down the system
        let mut watchdog = crate::device::WATCHDOG.get().lock();
        unsafe { watchdog.reset(0) };
    }
    halt();
}
