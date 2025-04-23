#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
extern crate display_client;

#[macro_use]
extern crate ulib;

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

    let img_data;
    let img_width;
    let img_height;

    if file.ends_with("qoi") {
        let Ok(file) = ulib::sys::openat(3, file.as_bytes(), 0, 0) else {
            println!("Error opening file {}", file);
            ulib::sys::exit(1);
        };

        let img = read_all(file);
        let (header, data) = gfx::format::qoi::read_qoi_header(&img).unwrap();
        let width = header.width as usize;
        let height = header.height as usize;
        let mut output = alloc::vec![0u32; width * height];
        gfx::format::qoi::decode_qoi(&header, data, &mut output, width);

        img_width = width;
        img_height = height;
        img_data = output;
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
                    if data.kind == proto::InputEvent::KIND_KEY {
                        match proto::ScanCode(data.data2) {
                            proto::ScanCode::ESCAPE | proto::ScanCode::Q => {
                                break 'outer;
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

        let fb = buf.video_mem();
        gfx::blit_buffer(
            fb, width, height, row_stride, 0, 0, &img_data, img_width, img_height, img_width,
        );

        buf.client_to_server_queue()
            .try_send(proto::Event {
                kind: proto::EventKind::PRESENT,
                data: [0; 7],
            })
            .ok();

        // signal(video)? for sync
        ulib::sys::sem_down(buf.get_sem_fd(buf.present_sem)).unwrap();

        unsafe { ulib::sys::sys_sleep_ms(1000) };
    }

    buf.client_to_server_queue()
        .try_send(proto::Event {
            kind: proto::EventKind::DISCONNECT,
            data: [0; 7],
        })
        .ok();
}
