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
SOURCE=$(realpath $0)
RELATIVE=$(realpath --relative-to=. $SOURCE)
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
lz4 -f --best --favor-decSpeed "${RELATIVE%.rs}.elf"
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
        unsafe { exit() };
        loop {}
    }

    pub struct Stdout;
    impl core::fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let msg = Message {
                tag: 0,
                objects: [0; 4],
            };
            let chan = ChannelDesc(1);
            unsafe { send(chan, &msg, s.as_ptr(), s.len()) };
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

    macro_rules! syscall {
        ($num:literal => $vis:vis fn $ident:ident ( $($arg:ident : $ty:ty),* $(,)? ) $( -> $ret:ty )?) => {
            core::arch::global_asm!(
                ".global {name}; {name}: svc #{num}; ret",
                name = sym $ident,
                num = const $num,
            );
            extern "C" {
                $vis fn $ident( $($arg: $ty,)* ) $(-> $ret)?;
            }
        };
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct ChannelDesc(pub u32);

    #[repr(C)]
    #[derive(Debug)]
    pub struct Message {
        pub tag: u64,
        pub objects: [u32; 4],
    }

    #[repr(C)]
    struct Channels(usize, usize);

    syscall!(1 => pub fn shutdown());
    syscall!(3 => pub fn yield_());
    syscall!(5 => pub fn spawn(pc: usize, sp: usize, flags: usize));
    syscall!(6 => pub fn exit());

    syscall!(7 => fn _channel() -> Channels);
    pub unsafe fn channel() -> (ChannelDesc, ChannelDesc) {
        let res = unsafe { _channel() };
        (ChannelDesc(res.0 as u32), ChannelDesc(res.1 as u32))
    }

    syscall!(8 => pub fn send(desc: ChannelDesc, msg: &Message, buf: *const u8, buf_len: usize) -> isize);
    syscall!(9 => pub fn recv(desc: ChannelDesc, msg: &mut Message, buf: *mut u8, buf_cap: usize) -> isize);
    syscall!(10 => pub fn recv_block(desc: ChannelDesc, msg: &mut Message, buf: *mut u8, buf_cap: usize) -> isize);
}

fn try_read_stdin(buf: &mut [u8]) -> isize {
    let mut msg = runtime::Message {
        tag: 0,
        objects: [0; 4],
    };
    let chan = runtime::ChannelDesc(1);
    let res = unsafe { runtime::recv_block(chan, &mut msg, buf.as_mut_ptr(), buf.len()) };
    res
}

fn main() {
    println!("Starting 🐚");

    let mut buf = [0; 4096];
    loop {
        match try_read_stdin(&mut buf) {
            -2 => print!("."),
            err @ (isize::MIN ..= -1) => {
                println!("Error: {err}");
            },
            data => {
                println!("input: {:?}", unsafe { core::str::from_utf8_unchecked(&buf[..data as usize]) });
            }
        }
    }

    unsafe { runtime::exit() };
}

// I'll just leave this here: */
