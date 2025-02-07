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
    extern "C" fn _start(x0: usize) -> ! {
        let channel = ChannelDesc(x0 as u32);
        crate::main(channel);
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
            unsafe { send_block(chan, &msg, s.as_bytes()) };
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
        if let Some(loc) = info.location() {
            println!("Panic at {}:{}:{}; {}", loc.file(), loc.line(), loc.column(), info.message());
        } else {
            println!("Panic; {}", info.message());
        }
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
    syscall!(5 => pub fn spawn(pc: usize, sp: usize, x0: usize, flags: usize));
    syscall!(6 => pub fn exit());
    
    syscall!(7 => fn _channel() -> Channels);
    pub unsafe fn channel() -> (ChannelDesc, ChannelDesc) {
        let res = unsafe { _channel() };
        (ChannelDesc(res.0 as u32), ChannelDesc(res.1 as u32))
    }

    const FLAG_NO_BLOCK: usize = 1 << 0;

    syscall!(8 => pub fn _send(desc: ChannelDesc, msg: &Message, buf: *const u8, buf_len: usize, flags: usize) -> isize);
    syscall!(9 => pub fn _recv(desc: ChannelDesc, msg: &mut Message, buf: *mut u8, buf_cap: usize, flags: usize) -> isize);

    pub unsafe fn send(desc: ChannelDesc, msg: &Message, buf: &[u8]) -> isize {
        unsafe { _send(desc, msg, buf.as_ptr(), buf.len(), FLAG_NO_BLOCK) }
    }
    pub unsafe fn send_block(desc: ChannelDesc, msg: &Message, buf: &[u8]) -> isize {
        unsafe { _send(desc, msg, buf.as_ptr(), buf.len(), 0) }
    }
    pub unsafe fn recv(desc: ChannelDesc, msg: &mut Message, buf: &mut [u8]) -> isize {
        unsafe { _recv(desc, msg, buf.as_mut_ptr(), buf.len(), FLAG_NO_BLOCK) }
    }
    pub unsafe fn recv_block(desc: ChannelDesc, msg: &mut Message, buf: &mut [u8]) -> isize {
        unsafe { _recv(desc, msg, buf.as_mut_ptr(), buf.len(), 0) }
    }
}

fn main(chan: runtime::ChannelDesc) {
    println!("Hello from child!");

    let status = unsafe { runtime::send_block(chan, &runtime::Message {
        tag: u64::from_be_bytes(*b"CHILD???"),
        objects: [0; 4],
    }, &[]) };

    unsafe { runtime::exit() };
}

// I'll just leave this here: */
