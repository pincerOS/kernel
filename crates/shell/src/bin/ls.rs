#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
#[macro_use]
extern crate ulib;

#[no_mangle]
fn main(argc: usize, argv: *const *const u8) {
    let cwd = ulib::sys::open(b".", 0).unwrap();

    // TODO: args should start at 1 for compatability
    if argc <= 0 {
        list_dir(cwd);
        ulib::sys::exit(0);
    }

    let argv_array = unsafe { core::slice::from_raw_parts(argv, argc) };
    let mut first = true;
    for arg in argv_array.iter().copied() {
        let arg = unsafe { core::ffi::CStr::from_ptr(arg) };
        let file = arg.to_bytes();
        let string = core::str::from_utf8(file).unwrap();

        let result_fd = ulib::sys::openat(cwd, file, 0, 0);
        if result_fd.is_err() {
            println!("ls: no such file or directory: {:?}", string);
            continue;
        }

        let fd = result_fd.unwrap();
        if argc > 1 {
            if !first {
                println!();
            }
            println!("{}:", string);
        }
        list_dir(fd);
        ulib::sys::close(fd).unwrap();

        first = false;
    }
}

fn list_dir(dir: u32) {
    let mut cookie = 0;
    let mut data_backing = [0u64; 8192 / 8];
    let data = cast_slice(&mut data_backing);

    fn cast_slice<'a>(s: &'a mut [u64]) -> &'a mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(s.as_mut_ptr().cast::<u8>(), s.len() * size_of::<u64>())
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub struct DirEntry {
        pub inode: u64,
        pub next_entry_cookie: u64,
        pub rec_len: u16,
        pub name_len: u16,
        pub file_type: u8,
        pub name: [u8; 3],
        // Name is an arbitrary size array; the record is always padded with
        // 0 bytes such that rec_len is a multiple of 8 bytes.
    }

    loop {
        match ulib::sys::pread(dir, data, cookie) {
            Err(e) => {
                println!("Error reading dir: {e}");
                ulib::sys::exit(1);
            },
            Ok(0) => break,
            Ok(len) => {
                let mut i = 0;
                while i < len as usize {
                    let slice = &data[i..];
                    assert!(slice.len() >= size_of::<DirEntry>());
                    let entry = unsafe { *slice.as_ptr().cast::<DirEntry>() };

                    let name_off = core::mem::offset_of!(DirEntry, name);
                    let name = &slice[name_off..][..entry.name_len as usize];
                    let name = core::str::from_utf8(name).unwrap();
                    println!("{}", name);
                    i += entry.rec_len as usize;
                    cookie = entry.next_entry_cookie;
                }
                if cookie == 0 {
                    break;
                }
            }
        }
    }
}
