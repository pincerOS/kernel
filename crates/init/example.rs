#!/usr/bin/env bash
#![doc = "<!-- Absolutely cursed hacks:
/*usr/bin/env true <<'END_BASH_COMMENT' # -->"]
#![no_std]
#![no_main]

mod runtime {
    #[rustfmt::skip]
    static _COMPILE_SCRIPT: () = { r##"
END_BASH_COMMENT
set -e
SOURCE=$(realpath "$0")
RELATIVE=$(realpath --relative-to=. "$SOURCE")
rustc --target=aarch64-unknown-none-softfloat \
    -C opt-level=2 -C panic=abort \
    -C strip=debuginfo \
    -C link-arg=-T"${SOURCE}" -C link-args='-zmax-page-size=0x1000' \
    "${SOURCE}" -o "${SOURCE%.rs}.elf"

SIZE=$(stat -c %s "${SOURCE%.rs}.elf" | python3 -c \
    "(lambda f:f(f,float(input()),0))\
     (lambda f,i,j:print('%.4g'%i,'BKMGTPE'[j]+'iB' if j else 'bytes')\
     if i<1024 else f(f,i/1024,j+1))"
)
echo "Built ${RELATIVE%.rs}.elf, file size ${SIZE}"
exit
    "##; };

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
        unsafe { core::arch::asm!("svc #6") }; // exit
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

    #[cfg(not(test))]
    #[panic_handler]
    fn panic_handler(info: &core::panic::PanicInfo) -> ! {
        println!("Panic: {}", info.message());
        loop {}
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
