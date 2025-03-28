#![no_std]
#![no_main]

#[macro_use]
extern crate ulib;

mod runtime;

use ulib::sys::FileDesc;

fn spawn_elf(fd: FileDesc) -> Result<FileDesc, usize> {
    let current_stack = current_sp();
    let target_pc = exec_child as usize;
    let arg = fd;

    let wait_fd = unsafe { ulib::sys::spawn(target_pc, current_stack, arg as usize, 0) };
    wait_fd
}

fn current_sp() -> usize {
    let sp: usize;
    unsafe { core::arch::asm!("mov {0}, sp", out(reg) sp) };
    sp
}

extern "C" fn exec_child(fd: FileDesc) -> ! {
    let flags = 0;
    let args = &[];
    let env = &[];
    let res = unsafe { ulib::sys::execve_fd(fd, flags, args, env) };
    println!("Execve failed: {:?}", res);
    ulib::sys::exit(1);
}

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    println!("Running in usermode! (parent)");

    let root_fd = 3;
    let path = b"example.elf";
    let file = ulib::sys::openat(root_fd, path, 0, 0).unwrap();

    // TODO: channels
    let child = spawn_elf(file).unwrap();

    let status = ulib::sys::wait(child).unwrap();

    println!("Child exited with status {}", status);

    // let msg = sys::Message {
    //     tag: 0xAABBCCDD,
    //     objects: [0; 4],
    // };
    // sys::send_block(child, &msg, b"Hello child!");

    // let mut buf = [0; 1024];

    // loop {
    //     let (len, msg) = sys::recv_block(child, &mut buf).unwrap();
    //     let data = &buf[..len];

    //     println!(
    //         "Received message from child; tag {:#x}, data {:?}",
    //         msg.tag,
    //         core::str::from_utf8(data).unwrap()
    //     );

    //     if data == b"shutdown" {
    //         break;
    //     }
    // }

    ulib::sys::shutdown();
}

#[macro_use]
#[doc(hidden)]
pub mod macros {
    #[repr(C)]
    pub struct AlignedAs<Align, Bytes: ?Sized> {
        #[allow(clippy::pub_underscore_fields)]
        pub _align: [Align; 0],
        pub bytes: Bytes,
    }
    #[doc(hidden)]
    #[macro_export]
    macro_rules! __include_bytes_align {
        ($align_ty:ty, $path:literal) => {{
            use $crate::macros::AlignedAs;
            static ALIGNED: &AlignedAs<$align_ty, [u8]> = &AlignedAs {
                _align: [],
                bytes: *include_bytes!($path),
            };
            &ALIGNED.bytes
        }};
    }
}

#[doc(inline)]
pub use crate::__include_bytes_align as include_bytes_align;
