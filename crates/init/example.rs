#!/usr/bin/env -S bash -c "rustc --target=aarch64-unknown-none-softfloat -C opt-level=2 -C panic=abort -C link-arg=-T${PWD}/example.rs -C link-args='-zmax-page-size=0x1000' -C strip=debuginfo example.rs -o example.elf"
#![doc = "<!-- Absolutely cursed hacks:
/* -->"]
#![no_std]
#![no_main]

mod runtime {
    #[rustfmt::skip]
    static _LINKER_SCRIPT: () = { r"*/
    ENTRY(_start);

    PHDRS {
        segment_main PT_LOAD;
    }

    SECTIONS {
        . = 0x300000;
        .text : { *(.text) *(.text*) } :segment_main
        .rodata : ALIGN(8) { *(.rodata) *(.rodata*) } :segment_main
        .data : { *(.data) *(.data.*) } :segment_main

        .bss (NOLOAD) : ALIGN(16) { *(.bss) *(.bss.*) } :segment_main
    }
    /*"; };

    #[no_mangle]
    extern "C" fn _start() -> ! {
        crate::main();
        loop {}
    }

    #[cfg(not(test))]
    #[panic_handler]
    fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
        loop {}
    }

    pub struct Stdout;
    impl core::fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            unsafe { core::arch::asm!("svc #4", in("x0") s.as_ptr(), in("x1") s.len()) };
            Ok(())
        }
    }
    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => {{
            use core::fmt::Write;
            write!($crate::runtime::Stdout, $($arg)*).ok();
        }};
    }
    #[macro_export]
    macro_rules! println {
        ($($arg:tt)*) => {{
            use core::fmt::Write;
            writeln!($crate::runtime::Stdout, $($arg)*).ok();
        }};
    }
}

fn main() {
    use core::arch::asm;

    for i in 0..10 {
        println!("Running in usermode! {}", i);
    }

    unsafe { asm!("svc #1") };
}

// I'll just leave this here: */
