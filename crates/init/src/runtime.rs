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
        unsafe { super::syscall::print(s.as_ptr(), s.len()) };
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
