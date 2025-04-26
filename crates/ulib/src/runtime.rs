unsafe extern "Rust" {
    fn main(argc: usize, argv: *const *const u8);
}

#[unsafe(no_mangle)]
#[cfg(not(feature = "newlib-stub"))]
extern "C" fn _start(argc: usize, argv: *const *const u8) -> ! {
    #[cfg(feature = "heap-impl")]
    unsafe {
        crate::heap_impl::init_heap()
    };

    unsafe { main(argc, argv) };
    crate::sys::exit(0);
}

#[cfg(not(test))]
#[cfg(not(feature = "test"))]
#[cfg(not(feature = "newlib-stub"))]
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

#[cfg(feature = "newlib-stub")]
#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    unsafe { core::arch::asm!("udf #2", options(noreturn)) }
}
