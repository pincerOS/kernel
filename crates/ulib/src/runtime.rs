extern "Rust" {
    fn main(chan: crate::sys::ChannelDesc);
}

#[no_mangle]
extern "C" fn _start(x0: usize) -> ! {
    unsafe { main(crate::sys::ChannelDesc(x0 as u32)) };
    unsafe { crate::sys::exit() };
    loop {}
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
    loop {}
}
