#![no_std]
#![no_main]

use core::ffi::{c_char, c_int, c_uchar};

use display_client::proto::{self, BufferHandle, SCANCODES};

#[macro_use]
extern crate ulib;

unsafe extern "C" {
    static DG_ScreenBuffer: *mut u32;

    fn doomgeneric_Create(argc: i32, argv: *const *const u8) -> i32;
    fn doomgeneric_Tick();
}

// #include "doomkeys.h"
// #include "m_argv.h"
// #include "doomgeneric.h"

// #include <stdio.h>
// #include <unistd.h>

// #include <stdbool.h>

// #define KEYQUEUE_SIZE 16

// static unsigned short s_KeyQueue[KEYQUEUE_SIZE];
// static unsigned int s_KeyQueueWriteIndex = 0;
// static unsigned int s_KeyQueueReadIndex = 0;

// static unsigned char convertToDoomKey(unsigned int key){
//   return KEY_ENTER;
// }

// static void addKeyToQueue(int pressed, unsigned int keyCode){
//   unsigned char key = convertToDoomKey(keyCode);

//   unsigned short keyData = (pressed << 8) | key;

//   s_KeyQueue[s_KeyQueueWriteIndex] = keyData;
//   s_KeyQueueWriteIndex++;
//   s_KeyQueueWriteIndex %= KEYQUEUE_SIZE;
// }
// static void handleKeyInput(){
//   // SDL_Event e;
//   // while (SDL_PollEvent(&e)){
//   //   if (e.type == SDL_QUIT){
//   //     puts("Quit requested");
//   //     atexit(SDL_Quit);
//   //     exit(1);
//   //   }
//   //   if (e.type == SDL_KEYDOWN) {
//   //     //KeySym sym = XKeycodeToKeysym(s_Display, e.xkey.keycode, 0);
//   //     //printf("KeyPress:%d sym:%d\n", e.xkey.keycode, sym);
//   //     addKeyToQueue(1, e.key.keysym.sym);
//   //   } else if (e.type == SDL_KEYUP) {256
//   //     //KeySym sym = XKeycodeToKeysym(s_Display, e.xkey.keycode, 0);
//   //     //printf("KeyRelease:%d sym:%d\n", e.xkey.keycode, sym);
//   //     addKeyToQueue(0, e.key.keysym.sym);
//   //   }
//   // }
// }

#[unsafe(no_mangle)]
extern "C" fn DG_Init() {}

#[unsafe(no_mangle)]
extern "C" fn DG_DrawFrame() {
    let size = 320 * 200 * 4;
    #[allow(static_mut_refs)]
    let doom_fb = unsafe {
        core::slice::from_raw_parts(DG_ScreenBuffer.cast::<u128>(), size / size_of::<u128>())
    };

    #[allow(static_mut_refs)]
    let window = unsafe { FRAMEBUF.as_mut().unwrap() };
    let framebuf = window.video_mem_u128();
    present(framebuf, doom_fb);

    window
        .client_to_server_queue()
        .try_send(proto::Event {
            kind: proto::EventKind::PRESENT,
            data: [0; 7],
        })
        .ok();

    // signal(video)? for sync
    ulib::sys::sem_down(window.get_sem_fd(window.present_sem)).unwrap();
    //   handleKeyInput();
}

#[unsafe(no_mangle)]
extern "C" fn DG_Quit() {
    #[allow(static_mut_refs)]
    let window = unsafe { FRAMEBUF.as_mut().unwrap() };
    window
        .client_to_server_queue()
        .try_send(proto::Event {
            kind: proto::EventKind::DISCONNECT,
            data: [0; 7],
        })
        .ok();
}

#[unsafe(no_mangle)]
extern "C" fn DG_SleepMs(ms: u32) {
    // println!("Sleep {ms}");
    unsafe { ulib::sys::sys_sleep_ms(ms as usize) };
}

#[unsafe(no_mangle)]
extern "C" fn DG_GetTicksMs() -> u32 {
    let t = unsafe { ulib::sys::sys_get_time_ms() as u32 };
    // println!("get time -> {t}");
    t
}

#[unsafe(no_mangle)]
unsafe extern "C" fn DG_GetKey(pressed_state: *mut c_int, out_doom_key: *mut c_uchar) -> c_int {
    use display_client::proto::{self, EventData, EventKind, ScanCode};

    #[allow(static_mut_refs)]
    let buf = unsafe { FRAMEBUF.as_mut().unwrap() };
    while let Some(ev) = buf.server_to_client_queue().try_recv() {
        match ev.kind {
            EventKind::INPUT => {
                let data = proto::InputEvent::parse(&ev).expect("TODO");
                if data.kind == proto::InputEvent::KIND_KEY {
                    let pressed = data.data1 == 1 || data.data1 == 3;
                    let code = ScanCode(data.data2);
                    const F1: u32 = ScanCode::F1.0;
                    const F10: u32 = ScanCode::F10.0;
                    const F11: u32 = ScanCode::F11.0;
                    const F12: u32 = ScanCode::F12.0;

                    let doom_key = match code {
                        ScanCode::ENTER => b'\r',
                        ScanCode::ESCAPE => b'\x1b',
                        ScanCode::BACKSPACE => b'\x7f',
                        ScanCode::TAB => b'\t',
                        ScanCode::SPACE => 0xA2, // Use action
                        c @ ScanCode(F1..=F10) => (0x80 + 0x3b + ((c.0) - ScanCode::F1.0)) as u8, // F1-F10
                        c @ ScanCode(F11..=F12) => (0x80 + 0x57 + ((c.0) - ScanCode::F11.0)) as u8, // F1-F10

                        // Arrows
                        ScanCode::RIGHT => 0xAE,
                        ScanCode::LEFT => 0xAC,
                        ScanCode::DOWN => 0xAF,
                        ScanCode::UP => 0xAD,

                        // Modifiers
                        ScanCode::LEFT_CTRL => 0xa3, // lctrl -> fire
                        ScanCode::RIGHT_CTRL => 0x80 + 0x1d,
                        ScanCode::LEFT_SHIFT | ScanCode::RIGHT_SHIFT => 0x80 + 0x36,
                        ScanCode::LEFT_ALT | ScanCode::RIGHT_ALT => 0x80 + 0x38,
                        _ => SCANCODES[code.0 as usize].unwrap_or('\0') as u8,
                    };

                    unsafe {
                        core::ptr::write(pressed_state, pressed as c_int);
                        core::ptr::write(out_doom_key, doom_key);
                    }
                    if pressed {
                        println!("Key: {:#x}, mapped {:#x}", code.0, doom_key);
                    }
                    return 1;
                }
            }
            _ => (),
        }
    }
    return 0;
}

#[unsafe(no_mangle)]
extern "C" fn DG_SetWindowTitle(_title: *const c_char) {
    // if (window != NULL){
    //   SDL_SetWindowTitle(window, title);
    // };
}

#[cfg(target_arch = "aarch64")]
fn memcpy128(dst: &mut [u128], src: &[u128]) {
    let len = dst.len();
    assert_eq!(len, src.len());
    assert!(len % 64 == 0);
    unsafe {
        core::arch::asm!(r"
        1:
        ldp {tmp1}, {tmp2}, [{src}, #0]
        stp {tmp1}, {tmp2}, [{dst}, #0]
        ldp {tmp1}, {tmp2}, [{src}, #16]
        stp {tmp1}, {tmp2}, [{dst}, #16]
        ldp {tmp1}, {tmp2}, [{src}, #32]
        stp {tmp1}, {tmp2}, [{dst}, #32]
        ldp {tmp1}, {tmp2}, [{src}, #48]
        stp {tmp1}, {tmp2}, [{dst}, #48]
        add {src}, {src}, #64 // TODO: figure out east way to use index increment
        add {dst}, {dst}, #64
        subs {count}, {count}, #4
        b.hi 1b // if count > 0, loop
        ",
        src = in(reg) src.as_ptr(),
        dst = in(reg) dst.as_mut_ptr(),
        count = in(reg) len,
        tmp1 = out(reg) _, tmp2 = out(reg) _,
        )
    }
}

#[cfg(not(target_arch = "aarch64"))]
fn memcpy128(dst: &mut [u128], src: &[u128]) {
    dst.copy_from_slice(src)
}

fn present(fb: &mut [u128], buf: &[u128]) {
    memcpy128(fb, &buf);
    // Force writes to go through
    core::hint::black_box(&mut *fb);
}

static mut FRAMEBUF: Option<BufferHandle> = None;

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    println!(
        "Running in usermode! (doom); buf: {:p}",
        &raw const DG_ScreenBuffer
    );

    let fb = display_client::connect(320, 200);
    unsafe {
        FRAMEBUF = Some(fb);
    }

    unsafe { doomgeneric_Create(0, core::ptr::null()) };

    loop {
        unsafe { doomgeneric_Tick() };
    }

    ulib::sys::exit(0);
}
