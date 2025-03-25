use crate::SpinLock;

pub static MouseActions: SpinLock<Mouse> = SpinLock::new(Mouse {
    buttons: 0,
    x: 0,
    y: 0,
    wheel: 0,
});

pub unsafe fn MouseAnalyze(buffer: *mut u8, _buffer_length: u32) {
    let buttons = unsafe { *buffer };
    let x = unsafe { *buffer.offset(1) };
    let y = unsafe { *buffer.offset(2) };
    let wheel = unsafe { *buffer.offset(3) };

    *MouseActions.lock() = Mouse {
        buttons,
        x: x as i8,
        y: y as i8,
        wheel: wheel as i8,
    };
}

#[derive(Clone)]
pub struct Mouse {
    pub buttons: u8,
    pub x: i8,
    pub y: i8,
    pub wheel: i8,
}
