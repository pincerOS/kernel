/**
 *
 * device/hid/keyboard.rs
 *  By Aaron Lo
 *   
 */
use core::sync::atomic::AtomicU64;

use super::iter_changed_bits;
use crate::sync::SpinLock;

#[derive(Debug)]
pub struct KeyEvent {
    pub code: u8,
    pub key: Key,
    pub pressed: bool,
}

pub struct KeyEventBuffer {
    recv_lock: SpinLock<()>,
    buffer: crate::ringbuffer::SpscOverwritingRingBuffer<4096, KeyEvent>,
}

impl KeyEventBuffer {
    pub fn poll(&self) -> Option<KeyEvent> {
        let _guard = self.recv_lock.lock();
        unsafe { self.buffer.try_recv() }
    }
}

// Reading from the queue has a lock, and writing to it is only done
// from within this module.
// TODO: ensure that KeyboardAnalyze can't run concurrently with itself?
unsafe impl Sync for KeyEventBuffer {}

pub static KEY_EVENTS: KeyEventBuffer = KeyEventBuffer {
    recv_lock: SpinLock::new(()),
    buffer: crate::ringbuffer::SpscOverwritingRingBuffer::new(),
};

static LAST_KEYBOARD_REPORT: AtomicU64 = AtomicU64::new(0);

pub unsafe fn KeyboardAnalyze(buffer: *mut u8, buffer_length: u32) {
    let buffer = unsafe { core::slice::from_raw_parts(buffer, buffer_length as usize) };
    if buffer.is_empty() {
        return;
    }

    let mut new_report: [u8; 8] = buffer.try_into().unwrap();

    // Sort the key array as descending to get a consistent ordering;
    // Keyboards may buffer events (..) into a single report.
    // https://usb.org/sites/default/files/hid1_11.pdf#page=83
    new_report[2..].sort_unstable_by(|a, b| b.cmp(a));

    let old_report = u64::to_ne_bytes(LAST_KEYBOARD_REPORT.swap(
        u64::from_ne_bytes(new_report),
        core::sync::atomic::Ordering::SeqCst,
    ));

    if old_report == new_report {
        return;
    }

    let emit_key = |keycode, pressed| {
        let key = ByteToKey(keycode);
        let event = KeyEvent {
            key,
            code: keycode,
            pressed,
        };
        unsafe { KEY_EVENTS.buffer.send_overwrite(event) };
    };

    let old_mods = old_report[0];
    let new_mods = new_report[0];

    iter_changed_bits(old_mods, new_mods, |idx, state| match idx {
        0 => emit_key(0xE0, state), // LEFT_CTRL
        1 => emit_key(0xE1, state), // LEFT_SHIFT
        2 => emit_key(0xE2, state), // LEFT_ALT
        3 => emit_key(0xE3, state), // LEFT_GUI
        4 => emit_key(0xE4, state), // RIGHT_CTRL
        5 => emit_key(0xE5, state), // RIGHT_SHIFT
        6 => emit_key(0xE6, state), // RIGHT_ALT
        7 => emit_key(0xE7, state), // RIGHT_GUI
        _ => unreachable!(),
    });

    let (mut i, mut j) = (2, 2);
    while i < 8 && j < 8 {
        let (a, b) = (new_report[i], old_report[j]);
        if a == b {
            // no change
            i += 1;
            j += 1;
        } else if a > b {
            // Both lists are descending, so if the new key is greater
            // than the old key, the new key was added (key down)
            emit_key(a, true);
            i += 1; // Skip the new key, continue comparing
        } else {
            // The new key is less than the old key, so the old key
            // was released.
            emit_key(b, false);
            j += 1; // Skip the old key, continue comparing
        }
    }
}

//Table is here: https://github.com/tmk/tmk_keyboard/wiki/USB%3A-HID-Usage-Table
pub fn ByteToKey(byte: u8) -> Key {
    match byte {
        0x04 => Key::A,
        0x05 => Key::B,
        0x06 => Key::C,
        0x07 => Key::D,
        0x08 => Key::E,
        0x09 => Key::F,
        0x0A => Key::G,
        0x0B => Key::H,
        0x0C => Key::I,
        0x0D => Key::J,
        0x0E => Key::K,
        0x0F => Key::L,
        0x10 => Key::M,
        0x11 => Key::N,
        0x12 => Key::O,
        0x13 => Key::P,
        0x14 => Key::Q,
        0x15 => Key::R,
        0x16 => Key::S,
        0x17 => Key::T,
        0x18 => Key::U,
        0x19 => Key::V,
        0x1A => Key::W,
        0x1B => Key::X,
        0x1C => Key::Y,
        0x1D => Key::Z,
        0x1E => Key::One,
        0x1F => Key::Two,
        0x20 => Key::Three,
        0x21 => Key::Four,
        0x22 => Key::Five,
        0x23 => Key::Six,
        0x24 => Key::Seven,
        0x25 => Key::Eight,
        0x26 => Key::Nine,
        0x27 => Key::Zero,
        0x28 => Key::Return,
        0x29 => Key::Escape,
        0x2A => Key::Backspace,
        0x2B => Key::Tab,
        0x2C => Key::Space,
        0xE0 => Key::LeftCtrl,
        0xE1 => Key::LeftShift,
        0xE2 => Key::LeftAlt,
        0xE3 => Key::LeftGui,
        0xE4 => Key::RightCtrl,
        0xE5 => Key::RightShift,
        0xE6 => Key::RightAlt,
        0xE7 => Key::RightGui,
        _ => Key::NotDefined,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    NotDefined,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Zero,
    Return,
    Escape,
    Backspace,
    Tab,
    Space,

    //Modifiers
    LeftCtrl,
    LeftShift,
    LeftAlt,
    LeftGui,
    RightCtrl,
    RightShift,
    RightAlt,
    RightGui,
}
