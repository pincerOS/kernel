#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
#[macro_use]
extern crate ulib;

#[no_mangle]
extern "C" fn main(argc: usize, argv: *const *const u8) -> ! {
    let argv_array = unsafe { core::slice::from_raw_parts(argv, argc) };
    let arg = argv_array[1];
    let arg = unsafe { core::ffi::CStr::from_ptr(arg) };
    let arg_bytes = arg.to_bytes();
    let arg_str = core::str::from_utf8(arg_bytes).unwrap();

    println!("| ping: {}", arg_str);
    //Add ping to the kernel

    ulib::sys::exit(0);
}
