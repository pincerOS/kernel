#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
extern crate ulib;

#[no_mangle]
fn main(argc: usize, argv: *const *const u8) -> ! {
    let mut offset = 0;
    let argv_array = unsafe { core::slice::from_raw_parts(argv, argc) };
    for (index, arg) in argv_array[1..].iter().copied().enumerate() {
        let arg = unsafe { core::ffi::CStr::from_ptr(arg) };
        let arg_bytes = arg.to_bytes();
        let arg_str = core::str::from_utf8(arg_bytes).unwrap();

        let bytes = arg_str.as_bytes();

        let length = ulib::sys::pwrite_all(1, bytes, offset).unwrap();
        offset += length as u64;

        if index == argv_array.len() - 2 {
            break;
        }
        ulib::sys::pwrite_all(1, b" ", offset).unwrap();
        offset += 1;
    }

    ulib::sys::pwrite_all(1, b"\n", offset).unwrap();

    ulib::sys::exit(0);
}
