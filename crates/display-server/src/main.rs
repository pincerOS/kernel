#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
extern crate display_proto as proto;

#[macro_use]
extern crate ulib;

use alloc::vec::Vec;
use proto::BufferHandle;
use ulib::sys::{dup3, mmap, recv_nonblock, send, FileDesc};

#[no_mangle]
fn main() {
    let fb = init_fb(640, 480);
    let server_socket = 13;
    handle_conns(fb, server_socket);
}

struct BufferInfo {
    fd: u32,
    present_sem_fd: u32,
    size: usize,
    mapped: *mut proto::BufferHeader,
}

fn handle_incoming(_msg: ulib::sys::Message, _buf: &[u8], resp_socket: FileDesc) -> BufferHandle {
    // TODO: proper listen + connect sockets
    // (this just broadcasts a response to all listeners and hopes that there aren't race conditions)

    let buffer = init_buffer();

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

    handle
}

fn init_buffer() -> BufferInfo {
    println!("init_buffer");
    let screen_size = (640, 480);
    let vmem_size = screen_size.0 * screen_size.1 * 4;

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
            width: screen_size.0 as u16,
            height: screen_size.1 as u16,
            row_stride: (screen_size.0 as u16 * 4),
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

fn present(fb: &mut Framebuffer, buf: &[u128]) {
    proto::memcpy128(&mut fb.data, &buf);
    // Force writes to go through
    core::hint::black_box(&mut *fb);
}

#[allow(unused)]
struct Framebuffer {
    fd: usize,
    width: usize,
    height: usize,
    stride: usize,
    data: &'static mut [u128],
}

fn init_fb(width: usize, height: usize) -> Framebuffer {
    let mut fb = ulib::sys::RawFB {
        fd: 0,
        size: 0,
        pitch: 0,
        width: 0,
        height: 0,
    };
    let buffer_fd = unsafe { ulib::sys::sys_acquire_fb(width, height, &mut fb) };
    println!("Buffer: {:?}", buffer_fd);
    println!(
        "buffer_size {}, width {}, height {}, pitch {}",
        fb.size, fb.width, fb.height, fb.pitch
    );

    let mapped = unsafe { ulib::sys::mmap(0, fb.size, 0, 0, buffer_fd as u32, 0).unwrap() };
    let framebuf = unsafe {
        core::slice::from_raw_parts_mut::<u128>(mapped.cast(), fb.size / size_of::<u128>())
    };

    Framebuffer {
        fd: buffer_fd as usize,
        width: fb.width,
        height: fb.height,
        stride: fb.pitch / size_of::<u32>(),
        data: framebuf,
    }
}

fn handle_conns(mut fb: Framebuffer, server_socket: FileDesc) {
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

    loop {
        let mut buf = [0u64; 32];
        while let Ok((len, msg)) = recv_nonblock(server_socket, bytemuck::bytes_of_mut(&mut buf)) {
            let buf = handle_incoming(msg, &bytemuck::bytes_of(&buf)[..len], server_socket);
            clients.push(buf);
        }

        let client_count = clients.len();
        for (i, buf) in clients.iter_mut().enumerate() {
            let mut ev_limit = 10;
            while let Some(msg) = buf.client_to_server_queue().try_recv() {
                if ev_limit == 0 {
                    break;
                }
                ev_limit -= 1;
                match msg.kind {
                    proto::EventKind::PRESENT => {
                        // HACK: treat newest window as active
                        if i == client_count - 1 {
                            use proto::EventData;
                            let proto::PresentEvent = proto::PresentEvent::parse(&msg).unwrap();

                            present(&mut fb, &buf.video_mem_u128());

                            // TODO: better sync system here? this will accumulate if client
                            // isn't waiting for acks
                            ulib::sys::sem_up(buf.get_sem_fd(buf.present_sem)).unwrap();

                            // TODO: better scheduling for this
                            handle_key_events(&mut *buf);
                        } else {
                            ulib::sys::sem_up(buf.get_sem_fd(buf.present_sem)).unwrap();
                        }
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

fn handle_key_events(buf: &mut proto::BufferHandle) {
    loop {
        let key = unsafe { ulib::sys::sys_poll_key_event() };
        if key < 0 {
            break;
        }
        let pressed = (key & 0x100) != 0;
        let code = key & 0xFF;
        let code = remap_keycode(code);
        let event = proto::InputEvent {
            kind: proto::InputEvent::KIND_KEY,
            data1: if pressed { 1 } else { 2 },
            data2: code.0,
            data3: 0,
            data4: 0,
        };
        buf.server_to_client_queue()
            .try_send_data(event)
            .map_err(drop)
            .unwrap();
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
