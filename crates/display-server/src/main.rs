#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
extern crate display_proto as proto;

#[macro_use]
extern crate ulib;

use alloc::vec::Vec;
use proto::BufferHandle;
use ulib::sys::{dup3, mmap, recv_nonblock, send, FileDesc};

mod framebuffer;

struct BufferInfo {
    fd: u32,
    present_sem_fd: u32,
    size: usize,
    mapped: *mut proto::BufferHeader,
}

struct Client {
    pos: (u16, u16),
    handle: BufferHandle,
}

#[no_mangle]
fn main() {
    let fb = framebuffer::init_fb(640, 480);
    let server_socket = 13;
    handle_conns(fb, server_socket);
}

fn handle_incoming(_msg: ulib::sys::Message, buf: &[u8], resp_socket: FileDesc) -> Client {
    // TODO: proper listen + connect sockets
    // (this just broadcasts a response to all listeners and hopes that there aren't race conditions)

    let width = u16::from_ne_bytes(buf[0..2].try_into().unwrap()); // TODO: don't panic
    let height = u16::from_ne_bytes(buf[2..4].try_into().unwrap()); // TODO: don't panic

    let buffer = init_buffer(width as usize, height as usize);

    let fds = [buffer.fd, buffer.present_sem_fd];
    let handle = unsafe { proto::BufferHandle::new(buffer.mapped, &fds) };

    let objects = [
        dup3(fds[0], u32::MAX, 0).unwrap(),
        dup3(fds[1], u32::MAX, 0).unwrap(),
        u32::MAX,
        u32::MAX,
    ];

    let buf = u64::to_le_bytes(buffer.size as u64);
    send(
        resp_socket,
        &ulib::sys::Message {
            tag: 0x100,
            objects,
        },
        &buf,
        0,
    );

    Client {
        handle,
        pos: (0, 0),
    }
}

fn init_buffer(width: usize, height: usize) -> BufferInfo {
    println!("init_buffer");
    let vmem_size = width * height * 4;

    let header_size = size_of::<proto::BufferHeader>().next_multiple_of(4096);
    let total_size = header_size + vmem_size;

    let fd = unsafe { ulib::sys::sys_memfd_create() } as u32;

    let buffer = unsafe { mmap(0, total_size, 0, 0, fd, 0) }.unwrap();
    println!("buffer allocated, {buffer:p}");

    let present_sem_fd = ulib::sys::sem_create(0).unwrap();
    let present_sem = proto::SemDescriptor(1);

    let header = proto::BufferHeader {
        version: 1.into(),
        magic: u32::from_ne_bytes(*b"SBUF").into(),
        kill_switch: 0.into(),
        last_words: [const { core::sync::atomic::AtomicU8::new(0) }; 32],

        meta: proto::GlobalMeta {
            segment_size: total_size as u32,
            vmem_offset: header_size as u32,
            vmem_size: vmem_size as u32,
        },

        client_to_server_queue: proto::EventQueue::new(),
        server_to_client_queue: proto::EventQueue::new(),

        video_meta: proto::VideoMeta {
            width: width as u16,
            height: height as u16,
            row_stride: (width as u16 * 4),
            bytes_per_pixel: 4,
            bit_layout: 0,
            present_ts: 0,
        },
        term_meta: proto::TermMeta { rows: 0, cols: 0 },

        present_sem,
    };

    println!("Writing header");
    let ptr = buffer.cast::<proto::BufferHeader>();
    unsafe { ptr.write(header) };
    println!("init done");

    BufferInfo {
        fd,
        size: total_size,
        present_sem_fd,
        mapped: ptr,
    }
}

fn handle_conns(mut fb: framebuffer::Framebuffer, server_socket: FileDesc) {
    let mut clients = Vec::new();

    // TODO: synchronization approach
    // Server:
    // - should be notified when clients send events
    // - should be able to multiplex clients on a single thread
    // Client
    // - must have a way to wait until the server has copied a frame
    //   out before updating it
    // - should have some way to wait for server events (+ present
    //   requests?)
    // TODO: vsync / rate management

    let mut to_remove = Vec::<usize>::new();

    let mut intermediate_fb = alloc::vec![0u128; fb.data.len()];

    let mut active = 0;

    loop {
        let mut buf = [0u64; 32];
        while let Ok((len, msg)) = recv_nonblock(server_socket, bytemuck::bytes_of_mut(&mut buf)) {
            let buf = handle_incoming(msg, &bytemuck::bytes_of(&buf)[..len], server_socket);
            active = clients.len(); // Select the newest window
            clients.push(buf);
        }

        let mut any_updated = false;

        for (i, client) in clients.iter_mut().enumerate() {
            let mut ev_limit = 10;
            while let Some(msg) = client.handle.client_to_server_queue().try_recv() {
                if ev_limit == 0 {
                    break;
                }
                ev_limit -= 1;
                match msg.kind {
                    proto::EventKind::PRESENT => {
                        // HACK: treat newest window as active
                        use proto::EventData;
                        let proto::PresentEvent = proto::PresentEvent::parse(&msg).unwrap();

                        match i {
                            0 => client.pos = (0, 0),
                            1 => client.pos = (320, 0),
                            2 => client.pos = (0, 240),
                            3 => client.pos = (320, 240),
                            _ => (),
                        }

                        let window_x = client.pos.0 as usize;
                        let window_y = client.pos.1 as usize;

                        let out = bytemuck::cast_slice_mut(&mut intermediate_fb);
                        let client_width = client.handle.video_meta.width as usize;
                        let client_height = client.handle.video_meta.height as usize;
                        let client_row_stride =
                            client.handle.video_meta.row_stride as usize / size_of::<u32>();
                        let client_fb = &*client.handle.video_mem();
                        blit_buffer(
                            out,
                            fb.width,
                            fb.height,
                            fb.stride,
                            window_x,
                            window_y,
                            client_fb,
                            client_width,
                            client_height,
                            client_row_stride,
                        );

                        any_updated = true;

                        // TODO: better sync system here? this will accumulate if client
                        // isn't waiting for acks
                        ulib::sys::sem_up(client.handle.get_sem_fd(client.handle.present_sem))
                            .unwrap();
                    }
                    proto::EventKind::DISCONNECT => {
                        // TODO: auto-disconnect on process exit?
                        println!("Client {} disconnected.", i);
                        to_remove.push(i);
                        break;
                    }
                    _ => {
                        // println!("{:?} {:?}", msg.kind.0, msg.data);
                    }
                }
            }
        }

        if any_updated {
            framebuffer::present(&mut fb, &intermediate_fb);
        }

        // TODO: better scheduling for this
        loop {
            let key = unsafe { ulib::sys::sys_poll_key_event() };
            if key < 0 {
                break;
            }
            let pressed = (key & 0x100) != 0;
            let code = key & 0xFF;
            let code = remap_keycode(code);

            match (code, pressed) {
                (proto::ScanCode::TAB, true) => {
                    // TODO: modifiers
                    if clients.is_empty() {
                        active = 0;
                    } else {
                        active = (active + 1) % clients.len();
                    }
                    println!("Switched active; active: {active}");
                    continue;
                }
                _ => (),
            }

            let active = active.min(clients.len().saturating_sub(1));
            if let Some(client) = clients.get_mut(active) {
                let event = proto::InputEvent {
                    kind: proto::InputEvent::KIND_KEY,
                    data1: if pressed { 1 } else { 2 },
                    data2: code.0,
                    data3: 0,
                    data4: 0,
                };
                let queue = client.handle.server_to_client_queue();
                queue.try_send_data(event).map_err(drop).unwrap();
            }
        }

        if !to_remove.is_empty() {
            to_remove.sort_by(|a, b| a.cmp(b).reverse());
        }
        for remove in to_remove.drain(..) {
            clients.remove(remove);
        }

        // TODO: composite and present
        // TODO: sleep until frame
        unsafe { ulib::sys::sys_sleep_ms(1) };
    }
}

fn blit_buffer(
    dst: &mut [u32],
    dst_w: usize,
    dst_h: usize,
    dst_stride: usize,
    dst_x: usize,
    dst_y: usize,
    src: &[u32],
    src_w: usize,
    src_h: usize,
    src_stride: usize,
) {
    let effective_width = src_w.min(dst_w.saturating_sub(dst_x));
    let effective_height = src_h.min(dst_h.saturating_sub(dst_y));
    for r in 0..effective_height {
        let src_row = &src[r * src_stride..][..effective_width];
        let dst_row = &mut dst[(r + dst_y) * dst_stride + dst_x..][..effective_width];
        dst_row.copy_from_slice(src_row);
    }
}

fn remap_keycode(code: isize) -> proto::ScanCode {
    use proto::ScanCode;
    match code {
        0x04 => ScanCode::A,
        0x05 => ScanCode::B,
        0x06 => ScanCode::C,
        0x07 => ScanCode::D,
        0x08 => ScanCode::E,
        0x09 => ScanCode::F,
        0x0A => ScanCode::G,
        0x0B => ScanCode::H,
        0x0C => ScanCode::I,
        0x0D => ScanCode::J,
        0x0E => ScanCode::K,
        0x0F => ScanCode::L,
        0x10 => ScanCode::M,
        0x11 => ScanCode::N,
        0x12 => ScanCode::O,
        0x13 => ScanCode::P,
        0x14 => ScanCode::Q,
        0x15 => ScanCode::R,
        0x16 => ScanCode::S,
        0x17 => ScanCode::T,
        0x18 => ScanCode::U,
        0x19 => ScanCode::V,
        0x1A => ScanCode::W,
        0x1B => ScanCode::X,
        0x1C => ScanCode::Y,
        0x1D => ScanCode::Z,
        0x1E => ScanCode::KEY1,
        0x1F => ScanCode::KEY2,
        0x20 => ScanCode::KEY3,
        0x21 => ScanCode::KEY4,
        0x22 => ScanCode::KEY5,
        0x23 => ScanCode::KEY6,
        0x24 => ScanCode::KEY7,
        0x25 => ScanCode::KEY8,
        0x26 => ScanCode::KEY9,
        0x27 => ScanCode::KEY0,
        0x28 => ScanCode::ENTER,
        0x29 => ScanCode::ESCAPE,
        0x2A => ScanCode::BACKSPACE,
        0x2B => ScanCode::TAB,
        0x2C => ScanCode::SPACE,
        0x2D => ScanCode::MINUS, // or underscore
        0x2E => ScanCode::EQUAL,
        0x2F => ScanCode::LEFT_BRACKET,
        0x30 => ScanCode::RIGHT_BRACKET,
        0x31 => ScanCode::BACKSLASH,
        // 0x32 is non-us
        0x33 => ScanCode::SEMICOLON,
        0x34 => ScanCode::APOSTROPHE,
        0x35 => ScanCode::BACKQUOTE,
        0x36 => ScanCode::COMMA,
        0x37 => ScanCode::PERIOD,
        0x38 => ScanCode::SLASH,
        0x39 => ScanCode::CAPS_LOCK,

        0x3A => ScanCode::F1,
        0x3B => ScanCode::F2,
        0x3C => ScanCode::F3,
        // ...
        0x45 => ScanCode::F12,
        // 0x46 => ScanCode::PRINT_SCREEN
        0x47 => ScanCode::SCROLL_LOCK,
        0x48 => ScanCode::PAUSE,
        0x49 => ScanCode::INSERT,
        0x4A => ScanCode::HOME,
        0x4B => ScanCode::PAGE_UP,
        0x4C => ScanCode::DELETE,
        0x4D => ScanCode::END,
        0x4E => ScanCode::PAGE_DOWN,
        0x4F => ScanCode::RIGHT,
        0x50 => ScanCode::LEFT,
        0x51 => ScanCode::DOWN,
        0x52 => ScanCode::UP,
        0x53 => ScanCode::NUM_LOCK,

        0xE0 => ScanCode::LEFT_CTRL,
        0xE1 => ScanCode::LEFT_SHIFT,
        0xE2 => ScanCode::LEFT_ALT,
        0xE3 => ScanCode::LEFT_SHIFT,
        0xE4 => ScanCode::RIGHT_CTRL,
        0xE5 => ScanCode::RIGHT_SHIFT,
        0xE6 => ScanCode::RIGHT_ALT,
        0xE7 => ScanCode::RIGHT_SUPER,
        _ => ScanCode::UNKNOWN,
    }
}
