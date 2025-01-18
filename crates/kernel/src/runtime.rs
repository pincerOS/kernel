use core::panic::PanicInfo;
use core::sync::atomic;

use crate::arch::halt;
use crate::uart;

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
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
    halt();
}
