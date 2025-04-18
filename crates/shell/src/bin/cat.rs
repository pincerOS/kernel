#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
#[macro_use]
extern crate ulib;

#[no_mangle]
extern "C" fn main(argc: usize, argv: *const *const u8) -> ! {

    for i in 0..argc {
        let arg = unsafe { *argv.add(i) };
        let len = (0..4096)
            .find(|&j| unsafe { *arg.add(j) } == 0)
            .expect("Null terminator not found in argv");
        
        let arg_str = unsafe {
            let bytes = core::slice::from_raw_parts(arg, len);
            core::str::from_utf8_unchecked(bytes)
        };

        let file = arg_str.as_bytes();

        let result_fd = ulib::sys::open(file, 0);
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
