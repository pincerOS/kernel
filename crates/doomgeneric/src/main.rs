#![no_std]
#![no_main]

use core::ffi::{c_char, c_int, c_uchar};

#[macro_use]
extern crate ulib;

unsafe extern "C" {
    static DG_ScreenBuffer: u32;

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
    //   // buf: DG_ScreenBuffer, DOOMGENERIC_RESX*sizeof(uint32_t)
    //   // SDL_UpdateTexture(texture, NULL, DG_ScreenBuffer, DOOMGENERIC_RESX*sizeof(uint32_t));

    // panic!();
    // unsafe { ulib::sys::sys_present((&raw const DG_ScreenBuffer).add(262), BUF.len()) }
    #[allow(static_mut_refs)]
    let doom_fb = unsafe {
        core::slice::from_raw_parts((&raw const DG_ScreenBuffer).byte_add(262 * 4).cast::<u128>(), BUF.len())
    };
    // println!("Presenting!");
    #[allow(static_mut_refs)]
    unsafe {
        // println!("framebuf: {:p}", *FRAMEBUF.as_mut().unwrap());
        present(FRAMEBUF.as_mut().unwrap(), doom_fb)
    };

    //   // SDL_RenderClear(renderer);
    //   // SDL_RenderCopy(renderer, texture, NULL, NULL);
    //   // SDL_RenderPresent(renderer);

    //   handleKeyInput();
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
unsafe extern "C" fn DG_GetKey(pressed_state: *mut c_int, doomKey: *mut c_uchar) -> c_int {
    let status = unsafe { ulib::sys::sys_poll_key_event() };
    if status < 0 {
        return 0;
    }
    let pressed = (status & 0x100) != 0;
    let code = status & 0xFF;
    let doom_key = match code as u8 {
        c @ (0x04..=0x1D) => b'a' + (c - 0x04),
        c @ (0x1E..=0x27) => b'1' + (c - 0x1E),
        0x28 => b'\r',
        0x29 => b'\x1b',
        0x2A => 0x7F, // backspace
        0x2B => b'\t',
        0x2C => 0xa2,                                    // space -> use
        c @ (0x3A..=0x43) => (0x80 + 0x3b + (c - 0x3A)), // F1-F10
        c @ (0x44..=0x45) => (0x80 + 0x57 + (c - 0x44)), // F11-F12
        // Arrows
        0x4F => 0xae,
        0x50 => 0xac,
        0x51 => 0xaf,
        0x52 => 0xad,
        // Modifiers
        0xE0 => 0xa3, // lctrl -> fire
        0xE4 => 0x80 + 0x1d,
        0xE1 | 0xE5 => 0x80 + 0x36,
        0xE2 | 0xE6 => 0x80 + 0x38,
        // 0xE3 | 0xE7 => (),
        i => i as u8,
    };
    if pressed {
        println!("Key: {:#x}, mapped {:#x}", code, doom_key);
    }

    unsafe {
        core::ptr::write(pressed_state, pressed as c_int);
        core::ptr::write(doomKey, doom_key);
    }
    1
}

#[unsafe(no_mangle)]
extern "C" fn DG_SetWindowTitle(title: *const c_char) {
    // if (window != NULL){
    //   SDL_SetWindowTitle(window, title);
    // };
}

static mut BUF: [u128; (640 * 480) / 4] = [0u128; (640 * 480) / 4];

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

    // unsafe {
    //     memory::clean_physical_buffer_for_device(
    //         self.buffer().as_mut_ptr().cast(),
    //         size_of_val(self.buffer),
    //     );
    // }

    // self.buffer.copy_from_slice(&self.alternate);
    // Force writes to go through
    core::hint::black_box(&mut *fb);
}

static mut FRAMEBUF: Option<&mut [u128]> = None;

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    println!(
        "Running in usermode! (doom); buf: {:p}",
        &raw const DG_ScreenBuffer
    );

    let mut fb = ulib::sys::RawFB {
        fd: 0,
        size: 0,
        pitch: 0,
        width: 0,
        height: 0,
    };
    let mut buffer_fd = unsafe { ulib::sys::sys_acquire_fb(640, 480, &mut fb) };
    println!("Buffer: {:?}", buffer_fd);
    println!("buffer_size {}, width {}, height {}, pitch {}", fb.size, fb.width, fb.height, fb.pitch);

    let mapped = unsafe { ulib::sys::mmap(0, fb.size, 0, 0, buffer_fd as u32, 0).unwrap() };
    let framebuf =
        unsafe { core::slice::from_raw_parts_mut::<u128>(mapped.cast(), fb.size / size_of::<u128>()) };

    // let vaddr = ulib::sys::mmap(0x180_0000, buffer_size, 0, 0, 0, 0).unwrap();
    // unsafe { ulib::sys::map_physical(vaddr, buffer_ptr) }.unwrap();
    // let ptr = vaddr as *mut u128;
    // assert!(ptr.is_aligned());

    // let array_elems = buffer_size / size_of::<u128>();
    // let array = unsafe { core::slice::from_raw_parts_mut(ptr, array_elems) };
    // Surface::new(array, width, height, pitch / 4);

    #[allow(static_mut_refs)]
    unsafe {
        BUF.fill(0xFFFF00FF00000000FFFF00FF00000000)
    };
    #[allow(static_mut_refs)]
    unsafe {
        present(framebuf, &mut BUF)
    };
    println!("framebuf: {:p}", framebuf);
    // unsafe { ulib::sys::sys_present(BUF.as_ptr(), BUF.len()) }

    #[allow(static_mut_refs)]
    unsafe {
        FRAMEBUF = Some(framebuf);
        println!("framebuf: {:p}", *FRAMEBUF.as_mut().unwrap());
    }

    // present(mapped.cast)
    // array.fill(0xFFFF00FF00000000FFFF00FF00000000);
    // core::hint::black_box(&mut *array);

    unsafe { doomgeneric_Create(0, core::ptr::null()) };

    loop {
        unsafe { doomgeneric_Tick() };
    }

    ulib::sys::exit(0);
}
