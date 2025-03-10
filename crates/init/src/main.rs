#![no_std]
#![no_main]

#[macro_use]
extern crate ulib;

mod runtime;

use ulib::sys;

static ARCHIVE: &[u8] = include_bytes_align!(u32, "../fs.arc");

// Define IPC message tags
const CMD_EXIT: u64 = 0x0000000000000001;
const CMD_CAT: u64 = 0x0000000000000002;
const CMD_LS: u64 = 0x0000000000000003;
const CMD_CD: u64 = 0x0000000000000004;
const CMD_EXEC: u64 = 0x0000000000000005;
const CMD_PWD: u64 = 0x0000000000000006;

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

fn resolve_path<'a>(path: &'a str, current_dir: &[u8], buf: &'a mut [u8]) -> (usize, &'a [u8]) {
    let mut path_for_lookup = path.as_bytes();
    let mut len = 0;

    if path.starts_with('/') {
        buf[..path.len()].copy_from_slice(path.as_bytes());
        len = path.len();
        path_for_lookup = &path_for_lookup[1..];
    } else if path.starts_with("./") {
        if !current_dir.is_empty() {
            buf[..current_dir.len()].copy_from_slice(current_dir);
            len = current_dir.len();

            if current_dir[current_dir.len() - 1] != b'/' {
                buf[len] = b'/';
                len += 1;
            }
        }

        let file_bytes = path[2..].as_bytes();
        buf[len..len + file_bytes.len()].copy_from_slice(file_bytes);
        len += file_bytes.len();

        path_for_lookup = &path.as_bytes()[2..];
    } else {
        if !current_dir.is_empty() {
            buf[..current_dir.len()].copy_from_slice(current_dir);
            len = current_dir.len();

            if current_dir[current_dir.len() - 1] != b'/' {
                buf[len] = b'/';
                len += 1;
            }
        }

        let path_bytes = path.as_bytes();
        buf[len..len + path_bytes.len()].copy_from_slice(path_bytes);
        len += path_bytes.len();

        path_for_lookup = if buf[0] == b'/' && len > 1 {
            &buf[1..len]
        } else {
            &buf[..len]
        };
    }

    (len, path_for_lookup)
}

#[no_mangle]
pub extern "C" fn main() {
    let archive = match initfs::Archive::load(ARCHIVE) {
        Ok(archive) => archive,
        Err(err) => {
            println!("Failed to initialize archive: {:?}", err);
            return;
        }
    };

    let mut current_dir_buf = [0u8; 512];
    let mut current_dir_len = 1;
    current_dir_buf[0] = b'/';

    println!("Init process starting...");

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

        match msg.tag {
            CMD_EXIT => {
                println!("Child requested exit");
                break;
            }
            CMD_CAT => {
                let file_path = match core::str::from_utf8(data) {
                    Ok(s) => s,
                    Err(_) => {
                        sys::send_block(child, &msg, b"Invalid filename");
                        continue;
                    }
                };

                let mut full_path_buf = [0u8; 512];
                let (_, path_for_lookup) = resolve_path(
                    file_path,
                    &current_dir_buf[..current_dir_len],
                    &mut full_path_buf,
                );

                match archive.find_file(path_for_lookup) {
                    Some((_, file_idx)) => {
                        let mut file_buf = [0; 0x100000];

                        // TODO: Check read permissions for the file

                        match archive.read_file(file_idx, &mut file_buf) {
                            Ok(file_data) => {
                                sys::send_block(child, &msg, file_data);
                            }
                            Err(_) => {
                                sys::send_block(child, &msg, b"Error reading file");
                            }
                        }
                    }
                    None => {
                        sys::send_block(child, &msg, b"File not found");
                    }
                }
            }
            CMD_LS => {
                let cur_dir = &current_dir_buf[..current_dir_len];

                let dir_result = if cur_dir.len() == 1 && cur_dir[0] == b'/' {
                    match archive.find_file(b"") {
                        Some(result) => Some(result),
                        None => archive.find_file(b"/"),
                    }
                } else if cur_dir[0] == b'/' {
                    archive.find_file(&cur_dir[1..])
                } else {
                    archive.find_file(cur_dir)
                };

                match dir_result {
                    Some((dir_idx, _)) => {
                        match archive.list_dir(dir_idx) {
                            Ok(entries) => {
                                // Create a response buffer instead of using Vec
                                let mut response_buf = [0u8; 4096];
                                let mut response_len = 0;

                                for (_, file) in entries {
                                    if let Some(name) = archive.get_file_name(file) {
                                        // Check if we have enough space
                                        if response_len + name.len() + 1 <= response_buf.len() {
                                            response_buf[response_len..response_len + name.len()]
                                                .copy_from_slice(name);
                                            response_len += name.len();
                                            response_buf[response_len] = b'\n';
                                            response_len += 1;
                                        }
                                    }
                                }
                                sys::send_block(child, &msg, &response_buf[..response_len]);
                            }
                            Err(_) => {
                                sys::send_block(child, &msg, b"Error listing directory");
                            }
                        }
                    }
                    None => {
                        sys::send_block(child, &msg, b"Directory not found");
                    }
                }
            }
            CMD_CD => {
                let dir_path = match core::str::from_utf8(data) {
                    Ok(s) => s,
                    Err(_) => {
                        sys::send_block(child, &msg, b"Invalid directory name");
                        continue;
                    }
                };

                if dir_path == ".." {
                    let cur_dir = &current_dir_buf[..current_dir_len];
                    if cur_dir.len() <= 1 || (cur_dir.len() == 1 && cur_dir[0] == b'/') {
                        current_dir_buf[0] = b'/';
                        current_dir_len = 1;
                    } else if let Some(pos) = cur_dir.iter().rposition(|&c| c == b'/') {
                        if pos == 0 {
                            current_dir_len = 1;
                        } else {
                            current_dir_len = pos;
                        }
                    } else {
                        // no slash found, set to root
                        current_dir_buf[0] = b'/';
                        current_dir_len = 1;
                    }
                    sys::send_block(child, &msg, b"");
                    continue;
                }

                if dir_path == "." || dir_path.is_empty() {
                    sys::send_block(child, &msg, b"");
                    continue;
                }

                if dir_path == "/" {
                    current_dir_buf[0] = b'/';
                    current_dir_len = 1;
                    sys::send_block(child, &msg, b"");
                    continue;
                }

                let mut new_path_buf = [0u8; 512];
                let (new_path_len, path_for_lookup) = resolve_path(
                    dir_path,
                    &current_dir_buf[..current_dir_len],
                    &mut new_path_buf,
                );

                match archive.find_file(path_for_lookup) {
                    Some((dir_idx, file)) => {
                        match archive.list_dir(dir_idx) {
                            Ok(_) => {
                                current_dir_buf[..new_path_len]
                                    .copy_from_slice(&new_path_buf[..new_path_len]);
                                current_dir_len = new_path_len;
                                sys::send_block(child, &msg, b"");
                            }
                            Err(_) => {
                                sys::send_block(child, &msg, b"Not a directory");
                            }
                        }
                    }
                    None => {
                        sys::send_block(child, &msg, b"Directory not found");
                    }
                }
            }
            CMD_EXEC => {
                let prog_path = match core::str::from_utf8(data) {
                    Ok(s) => s,
                    Err(_) => {
                        sys::send_block(child, &msg, b"Invalid program path");
                        continue;
                    }
                };

                let mut full_path_buf = [0u8; 512];
                let (_, path_for_lookup) = resolve_path(
                    prog_path,
                    &current_dir_buf[..current_dir_len],
                    &mut full_path_buf,
                );

                let path_for_lookup = if prog_path == "example.elf" || prog_path == "./example.elf"
                {
                    b"example.elf"
                } else {
                    path_for_lookup
                };

                match archive.find_file(path_for_lookup) {
                    Some((_, file_idx)) => {
                        let mut file_buf = [0; 0x100000]; // 1MB buffer for program
                        match archive.read_file(file_idx, &mut file_buf) {
                            Ok(file_data) => {
                                // TODO: when we have a security module, do checks
                                // 1. The file has execute permission bit set
                                // 2. The user has permission to execute files
                                // 3. The directory has search/execute permissions
                                // 4. No security policy prevents execution
                                // 5. The file is a valid ELF executable

                                match elf::Elf::new(file_data) {
                                    Ok(elf) => {
                                        let prog_chan = spawn_elf(elf);

                                        //  send initial message to the child
                                        let init_msg = sys::Message {
                                            tag: 0xAABBCCDD,
                                            objects: [0; 4],
                                        };

                                        let send_result = sys::send_block(
                                            prog_chan,
                                            &init_msg,
                                            b"Hello from shell!",
                                        );
                                        if send_result < 0 {
                                            sys::send_block(
                                                child,
                                                &msg,
                                                b"Failed to communicate with program",
                                            );
                                            continue;
                                        }

                                        // wait for a response from the program to ensure it's ready
                                        let mut response_buf = [0u8; 1024];
                                        match sys::recv_block(prog_chan, &mut response_buf) {
                                            Ok(_) => {
                                                sys::send_block(
                                                    child,
                                                    &msg,
                                                    b"Program executed successfully",
                                                );
                                            }
                                            Err(_) => {
                                                sys::send_block(
                                                    child,
                                                    &msg,
                                                    b"Program started but failed to respond",
                                                );
                                            }
                                        }
                                        // TODO: keep track of running programs, handle program output, and manage termination better
                                    }
                                    Err(_) => {
                                        sys::send_block(
                                            child,
                                            &msg,
                                            b"Not a valid executable program",
                                        );
                                    }
                                }
                            }
                            Err(_) => {
                                sys::send_block(child, &msg, b"Error reading program file");
                            }
                        }
                    }
                    None => {
                        sys::send_block(child, &msg, b"Program not found");
                    }
                }
            }
            CMD_PWD => {
                let cur_dir = &current_dir_buf[..current_dir_len];
                sys::send_block(child, &msg, cur_dir);
            }
            _ => {
                println!(
                    "Received unknown message from child; tag {:#x}, data {:?}",
                    msg.tag,
                    core::str::from_utf8(data).unwrap_or("[invalid utf8]")
                );
            }
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
