#![no_std]

#![cfg_attr(not(test), no_main)]

extern crate alloc;
#[macro_use]
extern crate ulib;

#[no_mangle]
pub fn main(argc: usize, argv: *const *const u8) -> ! {
    println!("cat: mem argc: {:p}", &argc as *const usize);
    println!("cat: argc: {}", argc);
    println!("cat: argv: {:p}", argv as *const u8);
    for i in 0..argc {
        let arg = unsafe { *argv.add(i) };
        println!("cat: mem argv[{}]: {:p}", i, arg as *const u8);
        //arg as string
        let len = (0..4096)
            .find(|&j| unsafe { *arg.add(j) } == 0)
            .expect("Null terminator not found in argv");
        
        let arg_str = unsafe {
            let bytes = core::slice::from_raw_parts(arg, len);
            core::str::from_utf8_unchecked(bytes)
        };

        println!("cat: argv[{}]: {}", i, arg_str);
    }
    ulib::sys::exit(0);
}