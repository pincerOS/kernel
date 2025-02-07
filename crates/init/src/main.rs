#![no_std]
#![no_main]

use core::mem::{transmute_copy, MaybeUninit};

mod runtime;
mod syscall;

static ELF_FILE: &[u8] = include_bytes_align!(u32, "../fs.arc");

fn spawn_thread<F>(func: F, stack: &'static mut [u128])
where
    F: FnOnce() + Send + 'static,
{
    println!("Spawning thread!");
    let (a, b, _c) = unsafe { stack.align_to_mut::<MaybeUninit<F>>() };
    let func_ptr;
    if size_of::<F>() == 0 {
        func_ptr = a.as_mut_ptr_range().end as usize;
    } else {
        let f = b.last_mut().unwrap();
        f.write(func);
        func_ptr = f as *mut _ as usize;
    }
    let sp = a.as_ptr_range().end;

    extern "C" fn spawn_inner<F>(ptr: *mut F)
    where
        F: FnOnce() + Send + 'static,
    {
        (unsafe { ptr.read() })();

        // TODO: this leaks the stack
        unsafe { syscall::exit() };
    }

    unsafe { syscall::spawn(spawn_inner::<F> as usize, sp as usize, func_ptr, 1) };
}

static mut FILE_DATA_BUFFER: [u8; 1 << 16] = [0; 1 << 16];
static mut FILE_DATA_BUFFER2: [u8; 1 << 16] = [0; 1 << 16];

fn spawn_elf(elf: elf::Elf<'_>) -> syscall::ChannelDesc {
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
        }
    }

    let (local, remote) = unsafe { syscall::channel() };

    let entry = elf.elf_header().e_entry();
    let new_sp = 0x80_0000;
    unsafe { syscall::spawn(entry as usize, new_sp, remote.0 as usize, 0) };

    local
}

// static mut ARCHIVE: Option<initfs::Archive> = None;

#[no_mangle]
pub extern "C" fn main() {
    let archive = initfs::Archive::load(ELF_FILE).unwrap();
    let archive2 = initfs::Archive::load(ELF_FILE).unwrap();
    // let archive = unsafe {
    //     ARCHIVE = Some(archive);
    //     ARCHIVE.as_ref().unwrap()
    // };

    let mut buf = [0; 0x10000];
    let (_, file) = archive.find_file(b"example.elf").unwrap();
    let file = archive.read_file(file, &mut buf).unwrap();

    let child = spawn_elf(elf::Elf::new(file).unwrap());

    let (filelocal, fileremote) = unsafe { syscall::channel() };

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct ReadAt {
        file_id: u32,
        amount: u32,
        offset: u64,
    }
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct Open {
        path_len: u32,
    }

    let stack = unsafe {
        static mut _THREAD_STACK: [u128; 1024] = [0; 1024];
        #[allow(static_mut_refs)]
        &mut _THREAD_STACK
    };

    spawn_thread(
        move || {
            println!("@ Running file server");
            // TODO: actually zero BSS?
            let mut data_buf = unsafe { &mut FILE_DATA_BUFFER };

            let mut buf = [0; 4096];
            let mut msg = syscall::Message {
                tag: 0,
                objects: [0; 4],
            };
            // TODO: need epoll/equivalent conn multiplexer
            loop {
                let len = unsafe { syscall::recv_block(filelocal, &mut msg, &mut buf) };
                println!("@ file server got message");
                match &u64::to_be_bytes(msg.tag) {
                    b"OPEN----" => {
                        assert!(size_of::<Open>() as isize <= len);
                        let data = unsafe { core::ptr::read_unaligned(&buf as *const _ as *const Open) };
                        let rest = &buf[size_of::<Open>()..len as usize];
                        assert_eq!(rest.len(), data.path_len as usize);
                        println!("@ file server opening file {:?}", rest);

                        if let Some((id, _header)) = archive.find_file(rest) {
                            println!("@ open successful, id {id}");
                            let msg = syscall::Message {
                                tag: u64::from_be_bytes(*b"OPENSUCC"),
                                objects: [0, 0, 0, 0],
                            };
                            let buf = u32::to_le_bytes(id as u32);
                            unsafe { syscall::send_block(filelocal, &msg, &buf) };
                        } else {
                            println!("@ open failed");
                            let msg = syscall::Message {
                                tag: u64::from_be_bytes(*b"OPENFAIL"),
                                objects: [0, 0, 0, 0],
                            };
                            unsafe { syscall::send_block(filelocal, &msg, &[]) };
                        }
                    }
                    b"READAT--" => {
                        assert!(size_of::<ReadAt>() as isize <= len);
                        let read = unsafe { core::ptr::read_unaligned(&buf as *const _ as *const ReadAt) };

                        if let Some(file) = archive.get_file(read.file_id as usize) {
                            assert!((file.size as usize) < data_buf.len());
                            let data = archive.read_file(file, data_buf).unwrap();
                            let msg = syscall::Message {
                                tag: u64::from_be_bytes(*b"DATA----"),
                                objects: [0, 0, 0, 0],
                            };
                            let slice = data.get(read.offset as usize .. (read.offset as usize + read.amount as usize).min(data.len()));
                            unsafe { syscall::send_block(filelocal, &msg, slice.unwrap_or(&[])) };
                        } else {
                            let msg = syscall::Message {
                                tag: u64::from_be_bytes(*b"NOFILE--"),
                                objects: [0, 0, 0, 0],
                            };
                            unsafe { syscall::send_block(filelocal, &msg, &[]) };
                        }

                        // TODO
                    }
                    m => panic!("unknown message {m:?}"),
                }
            }
        },
        stack,
    );

    let (proclocal, procremote) = unsafe { syscall::channel() };

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct Spawn {
        file_id: u32,
    }

    let stack = unsafe {
        static mut _THREAD_STACK: [u128; 1024] = [0; 1024];
        #[allow(static_mut_refs)]
        &mut _THREAD_STACK
    };
    spawn_thread(
        move || {
            println!("@ Running process server");
            let mut buf = [0; 4096];
            let mut msg = syscall::Message {
                tag: 0,
                objects: [0; 4],
            };
            // TODO: actually zero BSS?
            let data_buf = unsafe { &mut FILE_DATA_BUFFER2 };

            loop {
                let len = unsafe { syscall::recv_block(proclocal, &mut msg, &mut buf) };
                match &u64::to_be_bytes(msg.tag) {
                    b"SPAWN---" => {
                        // TODO: pass a capability for accessing a file from the fs?
                        // Or have opening a file create a seekable stream from the fs,
                        // then send the stream with the spawn message?

                        assert!(size_of::<Spawn>() as isize <= len);
                        let spawn = unsafe { core::ptr::read_unaligned(&buf as *const _ as *const Spawn) };

                        if let Some(file) = archive2.get_file(spawn.file_id as usize) {
                            assert!((file.size as usize) < data_buf.len());
                            let data = archive2.read_file(file, data_buf).unwrap();
                            let elf = elf::Elf::new(data).unwrap();
                            let child = spawn_elf(elf);
                            let msg = syscall::Message {
                                tag: u64::from_be_bytes(*b"SUCCESS-"),
                                objects: [child.0, 0, 0, 0],
                            };
                            unsafe { syscall::send_block(proclocal, &msg, &[]) };
                        } else {
                            let msg = syscall::Message {
                                tag: u64::from_be_bytes(*b"FAILURE-"),
                                objects: [0, 0, 0, 0],
                            };
                            unsafe { syscall::send_block(proclocal, &msg, &[]) };
                        }
                    }
                    _ => (),
                }
            }
        },
        stack,
    );

    // TODO: channels may need to be mpmc -- spsc in terms of processes,
    // but each process may have multiple threads

    let mut buf = [0; 4096];
    let mut msg = syscall::Message {
        tag: 0,
        objects: [0; 4],
    };
    loop {
        let len = unsafe { syscall::recv_block(child, &mut msg, &mut buf) };
        if msg.tag == u64::from_be_bytes(*b"CONNREQ-") {
            match &buf[..len as usize] {
                b"FILES---" => {
                    let msg = syscall::Message {
                        tag: u64::from_be_bytes(*b"CONNACPT"),
                        objects: [fileremote.0, 0, 0, 0],
                    };
                    unsafe { syscall::send_block(child, &msg, &[]) };
                }
                b"PROCS---" => {
                    let msg = syscall::Message {
                        tag: u64::from_be_bytes(*b"CONNACPT"),
                        objects: [procremote.0, 0, 0, 0],
                    };
                    unsafe { syscall::send_block(child, &msg, &[]) };
                }
                _ => {
                    let msg = syscall::Message {
                        tag: u64::from_be_bytes(*b"CONNDENY"),
                        objects: [0; 4],
                    };
                    unsafe { syscall::send_block(child, &msg, &[]) };
                }
            }
        }
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
