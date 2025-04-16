#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
#[macro_use]
extern crate ulib;

#[no_mangle]
fn main() {
    let file = b"file.txt";

    let fd = ulib::sys::open(file).unwrap();

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

    ulib::sys::exit(0);
}
