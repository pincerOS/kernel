#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
extern crate display_client;

#[macro_use]
extern crate ulib;

pub mod editor;
mod grid;
mod input;
mod vt100;

use display_client::proto;
use gfx::{color, format};

use ulib::sys::{pipe, pwrite_all, spawn_elf, ArgStr, FileDesc, SpawnArgs};

#[no_mangle]
fn main(argc: usize, argv: *const *const u8) {
    let argv_array = unsafe { core::slice::from_raw_parts(argv, argc) };
    let args = argv_array
        .iter()
        .copied()
        .map(|arg| unsafe { core::ffi::CStr::from_ptr(arg) }.to_bytes())
        .map(|arg| core::str::from_utf8(arg).unwrap())
        .collect::<alloc::vec::Vec<_>>();

    let remaining_args;
    let mut scale = 1;

    if let (Some("--scale"), Some(scale_str)) = (args.get(1).map(|s| *s), args.get(2)) {
        if let Ok(s) = scale_str.parse::<usize>() {
            scale = s;
        }
        remaining_args = &args[3..];
    } else if args.len() > 0 {
        remaining_args = &args[1..];
    } else {
        remaining_args = &[];
    }

    // Useful font resources:
    // - https://adafruit.github.io/web-bdftopcf/
    // - https://github.com/Tecate/bitmap-fonts

    let compressed_font = include_bytes_align!(u32, "../ctrld-fixed-10r.pcf.lz4");
    let size = lz4::frame::read_frame(compressed_font)
        .unwrap()
        .0
        .content_size()
        .unwrap();
    let mut font_data = alloc::vec![0; size as usize];
    let font_data = lz4::decode_into(compressed_font, &mut font_data).unwrap();
    let font = format::pcf::load_pcf(font_data).unwrap();

    let mut buf = display_client::connect(320 * scale as u16, 240 * scale as u16);

    let (width, height) = (
        buf.video_meta.width as usize,
        buf.video_meta.height as usize,
    );
    let row_stride = buf.video_meta.row_stride as usize / 4;

    buf.video_mem().fill(color::rgba(0, 0, 0, 255));

    let char_dims = font.dimensions();
    let hpad = 2;
    let vpad = 0;

    let fill_color = grid::Colors {
        fg: color::rgba(255, 255, 255, 255),
        bg: color::rgba(0, 0, 0, 0),
    };
    let mut grid = grid::CharGrid::new(
        (width, height),
        font.dimensions(),
        scale,
        hpad,
        vpad,
        fill_color,
    );

    let mut emulator = vt100::EmulatorState::new(grid.rows, grid.cols);

    let mut modifiers = editor::Modifiers::NONE;
    let mut editor = editor::LineEditor::new();

    let cwd = 3;
    let fd;

    if remaining_args.len() > 1 {
        fd = ulib::sys::openat(cwd, remaining_args[0].as_bytes(), 0, 0);
    } else {
        fd = ulib::sys::openat(cwd, b"/shell", 0, 0);
    }
    let fd = match fd {
        Ok(f) => f,
        Err(_) => {
            println!("Failed to open executable.");
            ulib::sys::exit(1);
        }
    };

    let (_shell, shell_stdin_tx, shell_stdout_rx) = {
        let (shell_stdin_rx, shell_stdin_tx) = pipe(0).unwrap();
        let (shell_stdout_rx, shell_stdout_tx) = pipe(0).unwrap();

        let shell = spawn_elf(&SpawnArgs {
            fd,
            args: &remaining_args
                .iter()
                .map(|a| ArgStr {
                    len: a.len(),
                    ptr: a.as_ptr(),
                })
                .collect::<alloc::vec::Vec<_>>(),
            stdin: Some(shell_stdin_rx),
            stdout: Some(shell_stdout_tx),
            stderr: Some(shell_stdout_tx),
        })
        .unwrap();
        (shell, shell_stdin_tx, shell_stdout_rx)
    };

    'outer: loop {
        let time_us = unsafe { ulib::sys::sys_get_time_ms() as u64 } * 1000;

        while let Some(ev) = buf.server_to_client_queue().try_recv() {
            match ev.kind {
                proto::EventKind::INPUT => {
                    handle_input(ev, &mut modifiers, &mut editor, shell_stdin_tx, time_us)
                }
                proto::EventKind::REQUEST_CLOSE => {
                    println!("Close requested, exiting.");
                    // TODO: kill child process?
                    break 'outer;
                }
                _ => (),
            }
        }

        {
            let data = buf.video_mem();
            data.fill(color::rgba(0, 0, 0, 255));

            let mut buf = [0; 4096];
            while let Ok(n @ 1..) = ulib::sys::pread(shell_stdout_rx, &mut buf, 0) {
                emulator.input(&buf[..n]);
            }

            emulator.update(grid.region(0, 0, grid.rows, grid.cols));

            let blink = ((time_us - editor.last_keypress) < 0_600_000
                || ((time_us - editor.last_keypress) % 1_200_000 < 600_000))
                && editor.selection_range().len() == 0;

            editor::draw_editor_into_console(&editor, &mut emulator, fill_color.fg, blink);

            render_grid(&grid, char_dims, scale, vpad, hpad, data, row_stride, &font);
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

fn render_grid(
    grid: &grid::CharGrid,
    char_dims: (usize, usize),
    scale: usize,
    vpad: usize,
    hpad: usize,
    data: &mut [u32],
    row_stride: usize,
    font: &format::pcf::LoadedPCF<'_>,
) {
    for r in 0..grid.rows {
        for c in 0..grid.cols {
            let ch = grid.chars[r * grid.cols + c];
            let colors = grid.colors[r * grid.cols + c];

            let y = char_dims.0 * scale * r + vpad;
            let x = char_dims.1 * scale * c + hpad;

            if (colors.bg & 0xFF000000) != 0 {
                for pr in 0..char_dims.0 * scale {
                    for pc in 0..char_dims.1 * scale {
                        let pixel = &mut data[(y + pr) * row_stride + x + pc];
                        *pixel = color::blend(colors.bg, *pixel);
                    }
                }
            }

            if ch != ' ' && (colors.fg & 0xFF000000) != 0 {
                font.draw_char(ch, data, y * row_stride + x, row_stride, scale, colors.fg);
            }
        }
    }
}

fn handle_input(
    ev: proto::Event,
    modifiers: &mut editor::Modifiers,
    editor: &mut editor::LineEditor,
    shell_stdin_tx: FileDesc,
    time_us: u64,
) {
    use proto::EventData;
    let data = proto::InputEvent::parse(&ev).expect("TODO");
    if data.kind == proto::InputEvent::KIND_KEY {
        match proto::ScanCode(data.data2) {
            proto::ScanCode::LEFT_SHIFT => modifiers.shift = data.data1 == 1,
            proto::ScanCode::RIGHT_SHIFT => modifiers.shift = data.data1 == 1,
            proto::ScanCode::LEFT_CTRL => modifiers.ctrl = data.data1 == 1,
            proto::ScanCode::RIGHT_CTRL => modifiers.ctrl = data.data1 == 1,
            proto::ScanCode::LEFT_ALT => modifiers.alt = data.data1 == 1,
            proto::ScanCode::RIGHT_ALT => modifiers.alt = data.data1 == 1,
            _ => (),
        };

        if let Some(ev) = input::remap_input(data, *modifiers) {
            if matches!(
                ev,
                editor::KeyEvent::Press(editor::Keypress::Function(
                    editor::Modifiers::NONE,
                    editor::FuncKey::Enter
                ))
            ) {
                pwrite_all(shell_stdin_tx, editor.buf.as_bytes(), 0).unwrap();
                pwrite_all(shell_stdin_tx, b"\r", 0).unwrap();
                editor.clear();
            } else {
                editor::editor_input(editor, ev, time_us);
            }
        }
    }
}
