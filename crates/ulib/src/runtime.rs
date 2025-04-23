unsafe extern "Rust" {
    fn main();
}

#[unsafe(no_mangle)]
extern "C" fn _start(_x0: usize) -> ! {
    #[cfg(feature = "heap-impl")]
    unsafe {
        crate::heap_impl::init_heap()
    };

    unsafe { main() };
    crate::sys::exit(0);
}

#[cfg(not(test))]
#[cfg(not(feature = "test"))]
#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    if let Some(loc) = info.location() {
        crate::println!(
            "Panic at {}:{}:{}; {}",
            loc.file(),
            loc.line(),
            loc.column(),
            info.message()
        );
    } else {
        crate::println!("Panic; {}", info.message());
    }
    unsafe { core::arch::asm!("udf #2", options(noreturn)) }
}
