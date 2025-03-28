use core::sync::atomic::{AtomicU8, Ordering};

use super::iter_changed_bits;
use crate::SpinLock;

#[derive(Debug)]
pub enum MouseEvent {
    Move {
        x: i8,
        y: i8,
    },
    Button {
        button: MouseButton,
        state: bool,
        all: MouseButtonState,
    },
    Wheel {
        delta: i8,
    },
}

#[derive(Debug)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    M4,
    M5,
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone)]
    pub struct MouseButtonState: u8 {
        const NONE = 0;
        const LEFT = 1 << 0;
        const RIGHT = 1 << 1;
        const MIDDLE = 1 << 2;
        const M4 = 1 << 3;
        const M5 = 1 << 4;
    }
}

pub struct MouseEventBuffer {
    recv_lock: SpinLock<()>,
    buffer: crate::ringbuffer::SpscOverwritingRingBuffer<4096, MouseEvent>,
}

impl MouseEventBuffer {
    pub fn poll(&self) -> Option<MouseEvent> {
        let _guard = self.recv_lock.lock();
        unsafe { self.buffer.try_recv() }
    }
}

// Reading from the queue has a lock, and writing to it is only done
// from within this module.
// TODO: ensure that MouseAnalyze can't run concurrently with itself?
unsafe impl Sync for MouseEventBuffer {}

pub static MOUSE_EVENTS: MouseEventBuffer = MouseEventBuffer {
    recv_lock: SpinLock::new(()),
    buffer: crate::ringbuffer::SpscOverwritingRingBuffer::new(),
};

pub static LAST_BUTTONS: AtomicU8 = AtomicU8::new(0);

pub unsafe fn MouseAnalyze(buffer: *mut u8, buffer_length: u32) {
    let buffer = unsafe { core::slice::from_raw_parts(buffer, buffer_length as usize) };

    if buffer.is_empty() {
        return;
    }

    let [buttons, x, y, wheel] = buffer.try_into().unwrap();
    let old_buttons = LAST_BUTTONS.swap(buttons, Ordering::SeqCst);

    let send = |ev| unsafe { MOUSE_EVENTS.buffer.send_overwrite(ev) };
    let button_state = MouseButtonState::from_bits_truncate(buttons);

    let emit_button = |button, state| {
        send(MouseEvent::Button {
            button,
            state,
            all: button_state,
        });
    };

    iter_changed_bits(old_buttons, buttons, |idx, state| match idx {
        0 => emit_button(MouseButton::Left, state),
        1 => emit_button(MouseButton::Right, state),
        2 => emit_button(MouseButton::Middle, state),
        3 => emit_button(MouseButton::M4, state),
        4 => emit_button(MouseButton::M5, state),
        _ => (),
    });

    if x != 0 || y != 0 {
        send(MouseEvent::Move {
            x: x as i8,
            y: y as i8,
        });
    }
    if wheel != 0 {
        send(MouseEvent::Wheel { delta: wheel as i8 });
    }
}
