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
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
