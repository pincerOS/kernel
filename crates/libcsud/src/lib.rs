#![no_std]
// #![no_main]

// use core::ffi::c_char;

// #[panic_handler]
// fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
//     unsafe { core::arch::asm!("udf #2", options(noreturn)) }
// }

// #[no_mangle]
// pub extern "C" fn init() {
//     Usb
// }

extern "C" {
    pub fn UsbInitialise(hcd_designware_base: *mut ()) -> i32;

    pub fn UsbCheckForChange();

    pub fn KeyboardCount() -> u32;
}
