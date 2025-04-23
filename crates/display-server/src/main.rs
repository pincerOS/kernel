#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate alloc;
extern crate display_proto as proto;

#[macro_use]
extern crate ulib;

use alloc::string::String;
use alloc::vec::Vec;
use proto::BufferHandle;
use thunderdome::{Arena, Index};
use ulib::sys::{dup3, mmap, recv_nonblock, send, FileDesc};

mod framebuffer;

struct BufferInfo {
    fd: u32,
    present_sem_fd: u32,
    size: usize,
    mapped: *mut proto::BufferHeader,
}

struct Client {
    handle: BufferHandle,
    title: String,
    present_ready: bool,
}

#[no_mangle]
fn main() {
    let fb = framebuffer::init_fb(1280, 720);
    let server_socket = 13;
    handle_conns(fb, server_socket);
}

fn handle_incoming(
    _msg: ulib::sys::Message,
    buf: &[u8],
    resp_socket: FileDesc,
    manager: &mut WindowManager,
) -> (Index, Client) {
    // TODO: proper listen + connect sockets
    // (this just broadcasts a response to all listeners and hopes that there aren't race conditions)

    let buf = bytemuck::try_from_bytes::<proto::ConnRequest>(buf).unwrap(); // TODO: don't panic

    let min_size = U16Vec2::new(buf.min_width, buf.min_height);
    let max_size = U16Vec2::new(buf.max_width, buf.max_height);
    let pref_size = U16Vec2::new(buf.width, buf.height);
    let (window, size) = manager.request_window(min_size, max_size, pref_size);
    let (width, height) = (size.x, size.y);

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

    let client = Client {
        handle,
        present_ready: false,
        title: String::new(),
    };
    (window, client)
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

use glam::{I16Vec2, U16Vec2};

struct Window {
    pos: U16Vec2,
    size: U16Vec2,
    _resizable: bool,
    client: Index,
}

struct WindowManager {
    default_pos: U16Vec2,
    screen_dims: U16Vec2,

    windows: Arena<Window>,
    layering: Vec<Index>,

    active: Option<Index>,
    mouse_down: bool,

    dragging: Option<(Index, U16Vec2)>,

    request_close: Vec<Index>,
}

enum HoveredState {
    None,
    Window(Index, U16Vec2),
    Titlebar(Index, U16Vec2),
    CloseButton(Index),
}

impl WindowManager {
    fn new(screen_dims: U16Vec2) -> Self {
        WindowManager {
            default_pos: U16Vec2::new(0, 0),
            screen_dims,
            windows: Arena::new(),
            layering: Vec::new(),
            active: None,
            dragging: None,
            mouse_down: false,
            request_close: Vec::new(),
        }
    }

    fn request_window(
        &mut self,
        min_size: U16Vec2,
        _max_size: U16Vec2,
        pref_size: U16Vec2,
    ) -> (Index, U16Vec2) {
        let size = pref_size.min(self.screen_dims).max(min_size);

        let overflow = self
            .default_pos
            .saturating_add(size)
            .cmpgt(self.screen_dims);
        if overflow.any() {
            // self.default_pos = U16Vec2::ZERO;
            let wrap = U16Vec2::new((!overflow.x) as u16, (!overflow.y) as u16);
            self.default_pos = self.default_pos * wrap;
        }
        let pos = self.default_pos;
        self.default_pos += U16Vec2::new(128, 128);

        let idx = self.windows.insert(Window {
            pos,
            size,
            _resizable: false,
            client: Index::DANGLING, // TODO
        });
        self.layering.push(idx);
        self.active = Some(idx);
        (idx, size)
    }

    fn remove_window(&mut self, window: Index) {
        // TODO: alt-f4 can leave windows active, but receiving no input until alt-tab
        self.windows.remove(window);
        self.layering.retain(|f| *f != window);
        if self.active == Some(window) {
            self.active = self.layering.last().map(|i| *i);
        }
    }

    fn select_window(&mut self, window: Index) {
        self.layering.retain(|f| *f != window);
        self.layering.push(window);
        self.active = Some(window);
    }

    fn alt_tab(&mut self) {
        if self.layering.is_empty() {
            self.active = None;
        } else {
            let old = self.active.map(|i| i.slot()).unwrap_or(0);
            for i in (old + 1..self.windows.capacity() as u32).chain(0..=old) {
                if let Some(idx) = self.windows.contains_slot(i) {
                    self.select_window(idx);
                    break;
                }
            }
            // active = (active + 1) % clients.len();
        }
        println!("Switched active; active: {:?}", self.active);
    }

    fn hovered(&self, cursor: U16Vec2) -> HoveredState {
        let title_height = 12;
        let close_width = 12;

        for (idx, window) in self.iter_windows_draw_order_rev() {
            let x_range = window.pos.x..window.pos.x.saturating_add(window.size.x);
            let y_top = window.pos.y;
            let y_split = window.pos.y.saturating_add(title_height);
            let y_bottom = y_split.saturating_add(window.size.y);

            let title_y_range = y_top..y_split;
            let window_y_range = y_split..y_bottom;
            if x_range.contains(&cursor.x) && title_y_range.contains(&cursor.y) {
                let close_x = window.pos.x + window.size.x - close_width;
                let close_range_x = close_x..close_x + close_width;
                let close_range_y = window.pos.y..window.pos.y + close_width;
                if close_range_x.contains(&cursor.x) && close_range_y.contains(&cursor.y) {
                    return HoveredState::CloseButton(idx);
                } else {
                    return HoveredState::Titlebar(idx, cursor - window.pos);
                }
            } else if x_range.contains(&cursor.x) && window_y_range.contains(&cursor.y) {
                let local_pos = cursor - window.pos - U16Vec2::new(0, title_height);
                return HoveredState::Window(idx, local_pos);
            }
        }
        HoveredState::None
    }

    fn mouse_move(&mut self, new_cursor: U16Vec2) {
        if let Some((idx, offset)) = self.dragging {
            self.windows[idx].pos = new_cursor.saturating_sub(offset);
        }
    }

    fn iter_windows_draw_order(&self) -> impl Iterator<Item = (Index, &Window)> {
        self.layering.iter().map(|i| (*i, &self.windows[*i]))
    }
    fn iter_windows_draw_order_rev(&self) -> impl Iterator<Item = (Index, &Window)> {
        self.layering.iter().rev().map(|i| (*i, &self.windows[*i]))
    }
}

fn load_image(img: &[u8]) -> (usize, usize, Vec<u32>) {
    let (header, data) = gfx::format::qoi::read_qoi_header(img).unwrap();
    let width = header.width as usize;
    let height = header.height as usize;
    // Decode image into a bitmap buffer
    let mut buf = alloc::vec![0u32; width * height];
    gfx::format::qoi::decode_qoi(&header, data, &mut buf, width);
    (width, height, buf)
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone)]
    pub struct Modifiers: u8 {
        const L_SHIFT = 1 << 0;
        const L_CTRL = 1 << 1;
        const L_ALT = 1 << 2;
        const L_SUPER = 1 << 3;
        const R_SHIFT = 1 << 4;
        const R_CTRL = 1 << 5;
        const R_ALT = 1 << 6;
        const R_SUPER = 1 << 7;

        const SHIFT = Self::L_SHIFT.bits() | Self::R_SHIFT.bits();
        const CTRL = Self::L_CTRL.bits() | Self::R_CTRL.bits();
        const ALT = Self::L_ALT.bits() | Self::R_ALT.bits();
        const SUPER = Self::L_SUPER.bits() | Self::R_SUPER.bits();
    }
}

impl Modifiers {
    pub fn shift_pressed(self) -> bool {
        self.intersects(Self::SHIFT)
    }
    pub fn ctrl_pressed(self) -> bool {
        self.intersects(Self::CTRL)
    }
    pub fn alt_pressed(self) -> bool {
        self.intersects(Self::ALT)
    }
    pub fn super_pressed(self) -> bool {
        self.intersects(Self::SUPER)
    }
}

fn handle_conns(mut fb: framebuffer::Framebuffer, server_socket: FileDesc) {
    let mut clients = Arena::new();

    let mut window_manager = WindowManager::new(U16Vec2::new(fb.width as u16, fb.height as u16));

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

    // Load image (included directly in the binary)
    let img = include_bytes!("../assets/cursor.qoi");
    let (cursor_width, cursor_height, cursor_buf) = load_image(img);

    let (close_width, close_height, close_buf) = load_image(include_bytes!("../assets/close.qoi"));
    let (close2_width, close2_height, close2_buf) =
        load_image(include_bytes!("../assets/close-pressed.qoi"));
    assert!(close_height == close2_height && close_width == close2_width);

    let compressed_font = include_bytes_align!(u32, "../../console/ctrld-fixed-10r.pcf.lz4");
    let size = lz4::frame::read_frame(compressed_font)
        .unwrap()
        .0
        .content_size()
        .unwrap();
    let mut font_data = alloc::vec![0; size as usize];
    let font_data = lz4::decode_into(compressed_font, &mut font_data).unwrap();
    let font = gfx::format::pcf::load_pcf(font_data).unwrap();

    let mut to_remove = Vec::<Index>::new();

    let mut intermediate_fb = alloc::vec![0u128; fb.data.len()];
    let mut cursor = U16Vec2::new(0, 0);

    let mut any_updated = true;

    let mut modifiers = Modifiers::empty();

    loop {
        let mut buf = [0u64; 32];
        while let Ok((len, msg)) = recv_nonblock(server_socket, bytemuck::bytes_of_mut(&mut buf)) {
            let (window, client) = handle_incoming(
                msg,
                &bytemuck::bytes_of(&buf)[..len],
                server_socket,
                &mut window_manager,
            );
            let idx = clients.insert(client);
            window_manager.windows[window].client = idx;
        }

        intermediate_fb.fill(0);

        // TODO: better scheduling for this
        loop {
            use proto::ScanCode;

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
                    window_manager.alt_tab();
                    continue;
                }
                (ScanCode::LEFT_SHIFT, p) => modifiers.set(Modifiers::L_SHIFT, p),
                (ScanCode::RIGHT_SHIFT, p) => modifiers.set(Modifiers::R_SHIFT, p),
                (ScanCode::LEFT_CTRL, p) => modifiers.set(Modifiers::L_CTRL, p),
                (ScanCode::RIGHT_CTRL, p) => modifiers.set(Modifiers::R_CTRL, p),
                (ScanCode::LEFT_ALT, p) => modifiers.set(Modifiers::L_ALT, p),
                (ScanCode::RIGHT_ALT, p) => modifiers.set(Modifiers::R_ALT, p),
                (ScanCode::LEFT_SUPER, p) => modifiers.set(Modifiers::L_SUPER, p),
                (ScanCode::RIGHT_SUPER, p) => modifiers.set(Modifiers::R_SUPER, p),
                _ => (),
            }

            if let Some(active) = window_manager.active {
                let window = &window_manager.windows[active];
                let client = clients.get_mut(window.client).unwrap();
                let event = proto::InputEvent {
                    kind: proto::InputEvent::KIND_KEY,
                    data1: if pressed { 1 } else { 2 },
                    data2: code.0,
                    data3: 0,
                    data4: 0,
                };
                let queue = client.handle.server_to_client_queue();
                queue.try_send_data(event).ok();
            }
        }

        let mut cursor_moved = false;

        while let Some(ev) = ulib::sys::poll_mouse_event() {
            if ev.kind == ulib::sys::EVENT_KEY {
                match ev.code {
                    0x1001..=0x1005 => {
                        let button = ev.code - 0x1000;
                        let pressed = ev.value != 0;
                        let hovered = window_manager.hovered(cursor);

                        if button == 1 && pressed {
                            match hovered {
                                HoveredState::None => (),
                                HoveredState::Titlebar(idx, offset) => {
                                    window_manager.dragging = Some((idx, offset));
                                    window_manager.select_window(idx);
                                }
                                HoveredState::Window(idx, _local_pos) => {
                                    window_manager.select_window(idx);
                                }
                                HoveredState::CloseButton(_idx) => {
                                    // window_manager.request_close.push(idx);
                                }
                            }
                            window_manager.mouse_down = true;
                        } else if button == 1 && !pressed {
                            match hovered {
                                HoveredState::CloseButton(idx) => {
                                    window_manager.request_close.push(idx);
                                }
                                _ => (),
                            }
                            window_manager.dragging = None;
                            window_manager.mouse_down = false;
                        }

                        match hovered {
                            HoveredState::Window(idx, local_pos) => {
                                let window = &window_manager.windows[idx];
                                let client = clients.get_mut(window.client).unwrap();
                                let event = proto::InputEvent {
                                    kind: proto::InputEvent::KIND_MOUSE,
                                    data1: if pressed { 2 } else { 3 },
                                    data2: local_pos.x as u32,
                                    data3: local_pos.y as u32,
                                    data4: button as u32,
                                };
                                let queue = client.handle.server_to_client_queue();
                                queue.try_send_data(event).ok();
                            }
                            HoveredState::None => (),
                            _ => (),
                        }
                    }
                    _ => (),
                }
            } else if ev.kind == ulib::sys::EVENT_RELATIVE {
                if ev.code == ulib::sys::REL_XY {
                    let y = (ev.value >> 16) as i16;
                    let x = (ev.value & 0xFFFF) as i16;
                    // println!("Mouse move: {}, {}", x, y);

                    cursor = (cursor.as_i16vec2() + I16Vec2::new(x, y))
                        .clamp(
                            I16Vec2::ZERO,
                            I16Vec2::new(fb.width as i16 - 1, fb.height as i16 - 1),
                        )
                        .as_u16vec2();
                    window_manager.mouse_move(cursor);
                    cursor_moved = true;

                    // TODO: dragging off window, still send mouse up to original window
                    let hovered = window_manager.hovered(cursor);
                    match hovered {
                        HoveredState::Window(idx, local_pos) => {
                            let window = &window_manager.windows[idx];
                            let client = clients.get_mut(window.client).unwrap();
                            let event = proto::InputEvent {
                                kind: proto::InputEvent::KIND_MOUSE,
                                data1: 1,
                                data2: local_pos.x as u32,
                                data3: local_pos.y as u32,
                                data4: 0,
                            };
                            let queue = client.handle.server_to_client_queue();
                            queue.try_send_data(event).ok();
                        }
                        _ => (),
                    }
                } else if ev.code == ulib::sys::REL_WHEEL {
                    // println!("Mouse wheel: {}", ev.value as i32);

                    let hovered = window_manager.hovered(cursor);
                    match hovered {
                        HoveredState::Window(idx, _local_pos) => {
                            let window = &window_manager.windows[idx];
                            let client = clients.get_mut(window.client).unwrap();
                            let event = proto::InputEvent {
                                kind: proto::InputEvent::KIND_SCROLL,
                                data1: ev.value as u32,
                                data2: 0,
                                data3: 0,
                                data4: 0,
                            };
                            let queue = client.handle.server_to_client_queue();
                            queue.try_send_data(event).ok();
                        }
                        _ => (),
                    }
                }
            }
        }

        for to_close in window_manager.request_close.drain(..) {
            let window = &window_manager.windows[to_close];
            let client = &mut clients[window.client];
            client
                .handle
                .server_to_client_queue()
                .try_send_data(proto::RequestCloseEvent)
                .ok();
            // TODO: if queue is full / etc, track non-acknowledgement and kill anyways
        }

        for (i, client) in clients.iter_mut() {
            use proto::EventData;
            let mut ev_limit = 10;
            while let Some(msg) = client.handle.client_to_server_queue().try_recv() {
                if ev_limit == 0 {
                    break;
                }
                ev_limit -= 1;
                match msg.kind {
                    proto::EventKind::PRESENT => {
                        let proto::PresentEvent = proto::PresentEvent::parse(&msg).unwrap();
                        client.present_ready = true;
                        any_updated = true;
                    }
                    proto::EventKind::TITLE => {
                        let proto::TitleEvent { len, data } =
                            proto::TitleEvent::parse(&msg).unwrap();
                        let buf = &data[..(len as usize).min(data.len())];
                        client.title = alloc::string::String::from_utf8_lossy(buf).into_owned();
                        println!("Updated title: {:?}", client.title);
                        any_updated = true;
                    }
                    proto::EventKind::DISCONNECT => {
                        // TODO: auto-disconnect on process exit?
                        println!("[disp] client {:?} disconnected.", i);
                        to_remove.push(i);
                        break;
                    }
                    _ => {
                        // println!("{:?} {:?}", msg.kind.0, msg.data);
                    }
                }
            }
        }

        let title_height = 12;
        let title_fg_color = 0xFF000000;
        let title_bg_color_active = 0xFFFFFFFF;
        let title_bg_color_inactive = 0xFFCCCCCC;

        let hovered = window_manager.hovered(cursor);

        for (idx, window) in window_manager.iter_windows_draw_order() {
            let client = &mut clients[window.client];
            let active = window_manager.active == Some(idx);

            let window_x = window.pos.x as usize;
            let window_y = window.pos.y as usize;

            if client.present_ready {
                let out = bytemuck::cast_slice_mut(&mut intermediate_fb);
                let client_width = client.handle.video_meta.width as usize;
                let client_height = client.handle.video_meta.height as usize;
                let client_row_stride =
                    client.handle.video_meta.row_stride as usize / size_of::<u32>();
                let client_fb = &*client.handle.video_mem();
                gfx::blit_buffer(
                    out,
                    fb.width,
                    fb.height,
                    fb.stride,
                    window_x,
                    window_y + title_height,
                    client_fb,
                    client_width,
                    client_height,
                    client_row_stride,
                );

                // TODO: better sync system here? this will accumulate if client
                // isn't waiting for acks
                ulib::sys::sem_up(client.handle.get_sem_fd(client.handle.present_sem)).unwrap();
            }

            // TODO: draw title bar, borders

            let bg_color = if active {
                title_bg_color_active
            } else {
                title_bg_color_inactive
            };

            let width = client.handle.video_meta.width as usize;
            let effective_width = width.min(fb.width.saturating_sub(window_x));
            for r in window_y..(window_y + title_height).min(fb.height) {
                let out = bytemuck::cast_slice_mut::<_, u32>(&mut intermediate_fb);
                let dst_row = &mut out[r * fb.stride + window_x..][..effective_width];
                dst_row.fill(bg_color);
            }

            let out = bytemuck::cast_slice_mut::<_, u32>(&mut intermediate_fb);
            let buf = &mut out
                [window_y * fb.stride..(window_y + title_height).min(fb.height) * fb.stride];
            let title_start = (fb.stride * 2) + window_x + 2;
            font.draw_string(
                &client.title,
                buf,
                title_start,
                Some(effective_width.saturating_sub(2)),
                fb.stride,
                1,
                title_fg_color,
            );

            // Draw close button

            let close_pressed = match hovered {
                HoveredState::CloseButton(i) => i == idx && window_manager.mouse_down,
                _ => false,
            };

            let buf = if close_pressed {
                &close2_buf
            } else {
                &close_buf
            };
            gfx::blit_buffer_blend(
                out,
                fb.width,
                fb.height,
                fb.stride,
                window_x + width - close_width,
                window_y,
                buf,
                close_width,
                close_height,
                close_width,
            );
        }

        let out = bytemuck::cast_slice_mut(&mut intermediate_fb);
        gfx::blit_buffer_blend(
            out,
            fb.width,
            fb.height,
            fb.stride,
            cursor.x as usize,
            cursor.y as usize,
            &cursor_buf,
            cursor_width,
            cursor_height,
            cursor_width,
        );

        if any_updated || cursor_moved {
            framebuffer::present(&mut fb, &intermediate_fb);
        }
        any_updated = false;

        if !to_remove.is_empty() {
            to_remove.sort_by(|a, b| a.cmp(b).reverse());
        }
        for remove in to_remove.drain(..) {
            if let Some(window) = window_manager
                .windows
                .iter()
                .find(|(_, w)| w.client == remove)
                .map(|(i, _)| i)
            {
                window_manager.remove_window(window);
            }
            clients.remove(remove);
            any_updated = true;
        }

        // TODO: composite and present
        // TODO: sleep until frame
        // TODO: intermediate buffers to prevent flickering for apps
        // that render slowly
        unsafe { ulib::sys::sys_sleep_ms(10) };
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
        0xE3 => ScanCode::LEFT_SUPER,
        0xE4 => ScanCode::RIGHT_CTRL,
        0xE5 => ScanCode::RIGHT_SHIFT,
        0xE6 => ScanCode::RIGHT_ALT,
        0xE7 => ScanCode::RIGHT_SUPER,
        _ => ScanCode::UNKNOWN,
    }
}
