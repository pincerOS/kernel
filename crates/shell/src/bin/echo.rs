#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
#[macro_use]
extern crate ulib;

#[no_mangle]
fn main(argc: usize, argv: *const *const u8) -> ! {

    let mut offset = 0;
    for i in 0..argc {
        let arg = unsafe { *argv.add(i) };
        let len = (0..4096)
            .find(|&j| unsafe { *arg.add(j) } == 0)
            .expect("Null terminator not found in argv");
        
        let arg_str = unsafe {
            let bytes = core::slice::from_raw_parts(arg, len);
            core::str::from_utf8_unchecked(bytes)
        };
        let bytes = arg_str.as_bytes();

        let length = ulib::sys::pwrite_all(1, bytes, offset).unwrap();
        offset += length as u64;
    }
    
    ulib::sys::exit(0);
}
