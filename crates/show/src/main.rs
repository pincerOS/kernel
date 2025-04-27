#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
extern crate display_client;

#[macro_use]
extern crate ulib;

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use display_client::proto;

use ulib::sys::FileDesc;

// TODO: stat for file length
fn read_all(fd: FileDesc) -> alloc::vec::Vec<u8> {
    let mut out = alloc::vec::Vec::new();
    let mut buf = [0u8; 512];
    let mut offset = 0;
    loop {
        match ulib::sys::pread(fd, &mut buf, offset) {
            Ok(0) => break,
            Ok(len) => {
                out.extend(&buf[..len]);
                offset += len as u64;
            }
            Err(e) => {
                println!("Error reading file: {e}");
                ulib::sys::exit(1);
            }
        }
    }
    out
}

fn load_image(file: u32) -> (usize, usize, Vec<u32>) {
    let img = read_all(file);
    let (header, data) = gfx::format::qoi::read_qoi_header(&img).unwrap();
    let width = header.width as usize;
    let height = header.height as usize;
    let mut output = alloc::vec![0u32; width * height];
    gfx::format::qoi::decode_qoi(&header, data, &mut output, width);

    (width, height, output)
}

fn list_dir(dir: u32) -> Vec<String> {
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

    let mut filenames = Vec::new();

    'outer: loop {
        match ulib::sys::pread(dir, data, cookie) {
            Err(e) => {
                println!("Error reading dir: {e}");
                ulib::sys::exit(1);
            }
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
                    filenames.push(name.to_owned());

                    i += entry.rec_len as usize;
                    cookie = entry.next_entry_cookie;
                }
                if cookie == 0 {
                    break 'outer;
                }
            }
        }
    }

    filenames
}

#[no_mangle]
fn main(argc: usize, argv: *const *const u8) {
    let argv_array = unsafe { core::slice::from_raw_parts(argv, argc) };
    let args = argv_array
        .iter()
        .copied()
        .map(|arg| unsafe { core::ffi::CStr::from_ptr(arg) }.to_bytes())
        .map(|arg| core::str::from_utf8(arg).unwrap())
        .collect::<alloc::vec::Vec<_>>();

    let file = args[1];

    let mut dir_fd = 3;
    let mut idx = 0;
    let mut files = Vec::new();

    let mut img_data;
    let mut img_width;
    let mut img_height;

    let mut last_idx = idx;

    if file.ends_with("qoi") {
        files.push(file.to_owned());
        let Ok(file) = ulib::sys::openat(3, file.as_bytes(), 0, 0) else {
            println!("Error opening file {}", file);
            ulib::sys::exit(1);
        };
        (img_width, img_height, img_data) = load_image(file);
    } else if let Ok(dir) = ulib::sys::openat(3, alloc::format!("{file}/").as_bytes(), 0, 0) {
        files = list_dir(dir);
        files.retain(|f| f.ends_with("qoi"));
        files.sort();
        println!("Loaded files: {:?}", files);
        dir_fd = dir;

        let Ok(file) = ulib::sys::openat(dir_fd, files[0].as_bytes(), 0, 0) else {
            println!("Error opening file {}", file);
            ulib::sys::exit(1);
        };
        (img_width, img_height, img_data) = load_image(file);
    } else {
        println!("Unknown file format, exiting.");
        ulib::sys::exit(1);
    }

    let mut buf = display_client::connect(img_width as u16, img_height as u16);
    buf.set_title(alloc::format!("show - {}", file).as_bytes());

    let (width, height) = (
        buf.video_meta.width as usize,
        buf.video_meta.height as usize,
    );
    let row_stride = buf.video_meta.row_stride as usize / 4;

    'outer: loop {
        while let Some(ev) = buf.server_to_client_queue().try_recv() {
            match ev.kind {
                proto::EventKind::INPUT => {
                    use proto::EventData;
                    let data = proto::InputEvent::parse(&ev).expect("TODO");
                    if data.kind == proto::InputEvent::KIND_KEY && data.data1 == 1 {
                        match proto::ScanCode(data.data2) {
                            proto::ScanCode::ESCAPE | proto::ScanCode::Q => {
                                break 'outer;
                            }
                            proto::ScanCode::RIGHT => {
                                idx = (idx + 1) % files.len();
                            }
                            proto::ScanCode::LEFT => {
                                idx = (idx + files.len() - 1) % files.len();
                            }
                            _ => (),
                        }
                    } else if data.kind == proto::InputEvent::KIND_MOUSE && data.data1 == 2 {
                        match data.data4 {
                            1 => {
                                // Mouse1 down
                                idx = (idx + 1) % files.len();
                            }
                            2 => {
                                // Mouse2 down
                                idx = (idx + files.len() - 1) % files.len();
                            }
                            _ => (),
                        }
                    }
                }
                proto::EventKind::REQUEST_CLOSE => {
                    break 'outer;
                }
                _ => (),
            }
        }

        idx = idx % files.len();
        if idx != last_idx {
            let start = unsafe { ulib::sys::sys_get_time_ms() };
            if let Ok(file) = ulib::sys::openat(dir_fd, files[idx].as_bytes(), 0, 0) {
                drop(img_data);
                (img_width, img_height, img_data) = load_image(file);
                let end = unsafe { ulib::sys::sys_get_time_ms() };
                println!("Loading image took {}ms", end - start);
                last_idx = idx;
            } else {
                println!("Error opening file {}", file);
                idx = last_idx;
            }
        }

        let x = width.saturating_sub(img_width) / 2;
        let y = height.saturating_sub(img_height) / 2;

        let fb = buf.video_mem();
        gfx::blit_buffer(
            fb, width, height, row_stride, x, y, &img_data, img_width, img_height, img_width,
        );

        buf.client_to_server_queue()
            .try_send(proto::Event {
                kind: proto::EventKind::PRESENT,
                data: [0; 7],
            })
            .ok();

        // signal(video)? for sync
        ulib::sys::sem_down(buf.get_sem_fd(buf.present_sem)).unwrap();
    }

    buf.client_to_server_queue()
        .try_send(proto::Event {
            kind: proto::EventKind::DISCONNECT,
            data: [0; 7],
        })
        .ok();
}
