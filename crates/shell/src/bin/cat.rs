#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
#[macro_use]
extern crate ulib;

#[no_mangle]
extern "C" fn main(argc: usize, argv: *const *const u8) -> ! {
    let argv_array = unsafe { core::slice::from_raw_parts(argv, argc) };
    for arg in argv_array[1..].iter().copied() {
        let arg = unsafe { core::ffi::CStr::from_ptr(arg) };
        let arg_bytes = arg.to_bytes();
        let arg_str = core::str::from_utf8(arg_bytes).unwrap();

        let file = arg_str.as_bytes();

        let result_fd = ulib::sys::openat(3, file, 0, 0);
        if result_fd.is_err() {
            println!("cat: no such file or directory: {:?}", arg_str);
            continue;
        }
        let fd = result_fd.unwrap();

        let mut buf = [0u8; 512];
        let mut offset = 0;
        loop {
            match ulib::sys::pread(fd, &mut buf, offset) {
                Ok(0) => break,
                Ok(len) => {
                    let data = &buf[..len];
                    let _ = ulib::sys::pwrite_all(1, &data, offset);

                    offset += len as u64;
                }
                Err(e) => {
                    println!("Error reading file: {e}");
                    ulib::sys::exit(1);
                }
            }
        }
        ulib::sys::close(fd).unwrap();
    }

    ulib::sys::exit(0);
}
