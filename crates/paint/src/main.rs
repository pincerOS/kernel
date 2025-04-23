#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
extern crate display_client;

#[macro_use]
extern crate ulib;

use core::ops::ControlFlow;

use display_client::proto;
use glam::IVec2;

fn draw_pixel(fb: &mut [u32], stride: usize, pos: IVec2, color: u32) {
    let height = fb.len() as i32 / stride as i32;
    let pos = pos.clamp(IVec2::ZERO, IVec2::new(stride as i32, height as i32));
    fb[(pos.y as usize * stride + pos.x as usize).clamp(0, fb.len() - 1)] = color;
}

fn get_pixel(fb: &mut [u32], stride: usize, pos: IVec2) -> u32 {
    let height = fb.len() as i32 / stride as i32;
    let pos = pos.clamp(IVec2::ZERO, IVec2::new(stride as i32, height as i32));
    fb[(pos.y as usize * stride + pos.x as usize).clamp(0, fb.len() - 1)]
}

fn draw_circle(fb: &mut [u32], stride: usize, pos: IVec2, color: u32, r: u32) {
    let r = r as i32;
    for x in -r ..= r {
        for y in -r ..= r {
            if x * x + y * y <= r {
                draw_pixel(fb, stride, pos + IVec2::new(x, y), color);
            }
        }
    }
}

#[no_mangle]
fn main(argc: usize, argv: *const *const u8) {
    let argv_array = unsafe { core::slice::from_raw_parts(argv, argc) };
    let _args = argv_array.iter().copied()
        .map(|arg| unsafe { core::ffi::CStr::from_ptr(arg) }.to_bytes())
        .map(|arg| core::str::from_utf8(arg).unwrap())
        .collect::<alloc::vec::Vec<_>>();

    let mut buf = display_client::connect(512, 384);
    buf.set_title("paint".as_bytes());

    let (width, height) = (
        buf.video_meta.width as usize,
        buf.video_meta.height as usize,
    );
    let row_stride = buf.video_meta.row_stride as usize / 4;

    let mut down = false;
    let mut pos = IVec2::new(0, 0);
    let mut radius = 3;
    let mut color = 0xFF000000;

    buf.video_mem().fill(0xFFFFFFFF);

    let color_palette = [
        0xFF000000,
        0xFFFFFFFF,
        0xFF464646,
        0xFFDCDCDC,
        0xFF787878,
        0xFFB4B4B4,
        0xFF990030,
        0xFF9C5A3C,
        0xFFED1C24,
        0xFFFFA3B1,
        0xFFFF7E00,
        0xFFE5AA7A,
        0xFFFFC20E,
        0xFFF5E49C,
        0xFFFFF200,
        0xFFFFF9BD,
        0xFFA8E61D,
        0xFFD3F9BC,
        0xFF22B14C,
        0xFF9DBB61,
        0xFF00B7EF,
        0xFF99D9EA,
        0xFF4D6DF3,
        0xFF709AD1,
        0xFF2F3699,
        0xFF546D8E,
        0xFF6F3198,
        0xFFB5A5D5,
    ];

    let fb = buf.video_mem();
    let palette_height = 16;
    let palette_elems = 14;
    for r in height - palette_height .. height {
        for c in 0..width {
            let idx = (c * palette_elems) / width;
            fb[r * row_stride + c] = color_palette[idx * 2];
        }
    }
    for r in height - 2 * palette_height .. height - palette_height {
        for c in 0..width {
            let idx = (c * palette_elems) / width;
            fb[r * row_stride + c] = color_palette[idx * 2 + 1];
        }
    }

    'outer: loop {
        while let Some(ev) = buf.server_to_client_queue().try_recv() {
            match ev.kind {
                proto::EventKind::INPUT => {
                    use proto::EventData;
                    let data = proto::InputEvent::parse(&ev).expect("TODO");
                    if let ControlFlow::Break(_) = handle_input(data, &mut buf, row_stride, &mut down, &mut pos, &mut color, &mut radius) {
                        break 'outer;
                    }
                }
                proto::EventKind::REQUEST_CLOSE => {
                    break 'outer;
                }
                _ => (),
            }
        }

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

fn handle_input(
    data: proto::InputEvent,
    buf: &mut proto::BufferHandle,
    row_stride: usize,
    down: &mut bool,
    pos: &mut IVec2,
    color: &mut u32,
    radius: &mut u32,
) -> ControlFlow<()> {
    if data.kind == proto::InputEvent::KIND_KEY {
        match proto::ScanCode(data.data2) {
            proto::ScanCode::ESCAPE | proto::ScanCode::Q => {
                return ControlFlow::Break(());
            },
            _ => (),
        }
    } else if data.kind == proto::InputEvent::KIND_MOUSE {
        let mode = data.data1;
        let x = data.data2 as i32;
        let y = data.data3 as i32;
        let new_pos = IVec2::new(x, y);
        let button = data.data4;
        match mode {
            proto::InputEvent::MODE_MOUSE_MOVE => {
                if *down {
                    let distance = (new_pos - *pos).abs().max_element();
                    // TODO: sqrt
                    for i in (0..=128).step_by((128 / distance.max(1) as usize).max(1)) {
                        let p = (*pos * (128 - i) + new_pos * i) / 128;
                        draw_circle(buf.video_mem(), row_stride, p, *color, *radius);
                    }
                }
            },
            proto::InputEvent::MODE_MOUSE_DOWN => {
                if button == 1 {
                    *down = true;
                    draw_circle(buf.video_mem(), row_stride, new_pos, *color, *radius);
                }
                if button == 3 {
                    *color = get_pixel(buf.video_mem(), row_stride, new_pos);
                }
            },
            proto::InputEvent::MODE_MOUSE_UP => {
                if button == 1 {
                    *down = false;
                }
            }
            _ => (),
        }
        *pos = new_pos;
    } else if data.kind == proto::InputEvent::KIND_SCROLL {
        // TODO: log scale using only integers
        *radius = radius.saturating_add_signed(data.data1 as i32);
    }
    ControlFlow::Continue(())
}
