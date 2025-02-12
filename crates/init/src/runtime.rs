use core::arch::global_asm;

// TODO: #[global_allocator]?

global_asm!(
    "
.section .text.entry
.global entry
.global halt

entry:
    bl main

halt:
    nop
1:  wfe
    b 1b
    "
);

#[cfg(not(test))]
#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    if let Some(loc) = info.location() {
        println!("Panic at {}:{}:{}; {}", loc.file(), loc.line(), loc.column(), info.message());
    } else {
        println!("Panic; {}", info.message());
    }
    loop {}
}
