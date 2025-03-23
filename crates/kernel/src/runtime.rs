#[cfg_attr(not(any(test, doc)), panic_handler)]
#[allow(dead_code)]
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
        println!(
            "Kernel panic at '{location}:{line}:{column}'\nmessage: {}",
            info.message()
        );
    }

    // TODO: write error message to a fixed location in memory and reset?

    crate::device::LED_OUT.get().put(0b00011000);

    {
        // Shut down the system
        let mut watchdog = crate::device::WATCHDOG.get().lock();
        unsafe { watchdog.reset(0) };
    }
    halt();
}
