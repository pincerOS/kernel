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

pub struct Stdout;
impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let msg = crate::syscall::Message {
            tag: 0,
            objects: [0; 4],
        };
        let chan = crate::syscall::ChannelDesc(1);
        unsafe { crate::syscall::send_block(chan, &msg, s.as_bytes()) };
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
