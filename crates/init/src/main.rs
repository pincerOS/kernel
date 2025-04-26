#![no_std]
#![no_main]

#[macro_use]
extern crate ulib;

mod runtime;

use ulib::sys::{spawn_elf, ArgStr};

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    println!("Running in usermode! (parent)");

    let (serve_a, serve_b) = ulib::sys::channel();

    ulib::sys::dup3(serve_a, 12, 0).unwrap();
    ulib::sys::dup3(serve_b, 13, 0).unwrap();
    ulib::sys::close(serve_a).unwrap();
    ulib::sys::close(serve_b).unwrap();

    let root_fd = 3;

    let path = b"display-server.elf";
    let file = ulib::sys::openat(root_fd, path, 0, 0).unwrap();
    spawn_elf(&ulib::sys::SpawnArgs {
        fd: file,
        stdin: None,
        stdout: None,
        stderr: None,
        args: &[ArgStr {
            len: path.len(),
            ptr: path.as_ptr(),
        }],
    })
    .unwrap();

    let path = b"shell";
    let file = ulib::sys::openat(root_fd, path, 0, 0).unwrap();

    let child = spawn_elf(&ulib::sys::SpawnArgs {
        fd: file,
        stdin: None,
        stdout: None,
        stderr: None,
        args: &[ArgStr {
            len: path.len(),
            ptr: path.as_ptr(),
        }],
    })
    .unwrap();

    let status = ulib::sys::wait(child).unwrap();

    println!("Child exited with status {}", status);

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
