#![no_std]
#![no_main]

#[macro_use]
extern crate ulib;

mod runtime;

use ulib::sys;

static ARCHIVE: &[u8] = include_bytes_align!(u32, "../fs.arc");

fn spawn_elf(elf: elf::Elf<'_>) -> sys::ChannelDesc {
    let phdrs = elf.program_headers().unwrap();

    for phdr in phdrs {
        let phdr = phdr.unwrap();
        if matches!(phdr.p_type, elf::program_header::Type::Load) {
            let data = elf.segment_data(&phdr).unwrap();
            let memsize = (phdr.p_memsz as usize).next_multiple_of(4096).max(4096);

            // TODO: mmap
            let addr = (phdr.p_vaddr as usize) as *mut u8;
            let mapping: &mut [u8] = unsafe { core::slice::from_raw_parts_mut(addr, memsize) };
            mapping[..data.len()].copy_from_slice(data);
            mapping[data.len()..].fill(0);
        }
    }

    let (local, remote) = sys::channel();

    let entry = elf.elf_header().e_entry();
    let new_sp = 0x100_0000;
    let x0 = remote.0 as usize;
    unsafe { sys::spawn(entry as usize, new_sp, x0, 0) };

    local
}

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    let archive = initfs::Archive::load(ARCHIVE).unwrap();

    let mut buf = [0; 0x18000];
    let (_, file) = archive.find_file(b"example.elf").unwrap();
    let file = archive.read_file(file, &mut buf).unwrap();

    println!("Running in usermode! (parent)");

    let elf = elf::Elf::new(file).unwrap();
    let child = spawn_elf(elf);

    let msg = sys::Message {
        tag: 0xAABBCCDD,
        objects: [0; 4],
    };
    sys::send_block(child, &msg, b"Hello child!");

    let mut buf = [0; 1024];

    loop {
        let (len, msg) = sys::recv_block(child, &mut buf).unwrap();
        let data = &buf[..len];

        println!(
            "Received message from child; tag {:#x}, data {:?}",
            msg.tag,
            core::str::from_utf8(data).unwrap()
        );

        if data == b"shutdown" {
            break;
        }
    }

    unsafe { sys::shutdown() };
    unreachable!();
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
