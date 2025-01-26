#![no_std]
#![no_main]

mod runtime;
mod syscall;

static ELF_FILE: &[u8] = include_bytes_align!(u32, "../fs.arc");

#[no_mangle]
pub extern "C" fn main() {
    let archive = initfs::Archive::load(ELF_FILE).unwrap();

    let mut buf = [0; 0x8000];
    let (_, file) = archive.find_file(b"example.elf").unwrap();
    let file = archive.read_file(file, &mut buf).unwrap();

    let elf = elf::Elf::new(file).unwrap();
    let phdrs = elf.program_headers().unwrap();

    for phdr in phdrs {
        let phdr = phdr.unwrap();
        if matches!(phdr.p_type, elf::program_header::Type::Load) {
            let data = &file[phdr.p_offset as usize..][..phdr.p_filesz as usize];

            let size = (phdr.p_memsz as usize).next_multiple_of(4096).max(4096);
            // TODO: mmap
            let addr = (phdr.p_vaddr as usize) as *mut u8;
            let mapping: &mut [u8] = unsafe { core::slice::from_raw_parts_mut(addr, size) };
            mapping[..data.len()].copy_from_slice(data);
        }
    }

    let entry = elf.elf_header().e_entry();
    let new_sp = 0x80_0000;
    unsafe { syscall::spawn(entry as usize, new_sp, 0) };

    for i in 0..10 {
        println!("Running in usermode! {}", i);
    }

    unsafe { syscall::exit() };
    unreachable!();
}

#[macro_use]
#[doc(hidden)]
pub mod macros {
    #[repr(C)]
    pub struct AlignedAs<Align, Bytes: ?Sized> {
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
