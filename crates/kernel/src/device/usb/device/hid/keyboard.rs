
use super::super::super::usbd::device::*;
use super::super::super::usbd::descriptors::*;
use super::super::super::usbd::usbd::*;

use alloc::vec::Vec;
use alloc::vec;
use alloc::boxed::Box;
use crate::device::system_timer::micro_delay;
use crate::device::usb::types::*;
use crate::device::usb::hcd::hub::*;
use crate::device::usb::usbd::pipe::*;
use crate::device::usb::usbd::request::*;

pub static mut KeyboardBuffer: Vec<Key> = vec![];

pub fn KeyboardAnalyze(buffer: *mut u8) {
    let mut keys = ModifierToKeys(unsafe { *buffer });

    //the second byte is alwasy 0
    for i in 2..8 {
        let key = ByteToKey(unsafe { *buffer.offset(i as isize) });
        if !matches!(key, Key::NotDefined) {
            keys.push(key);
        }
    }
    // println!("{:?}", keys);
    unsafe { KeyboardBuffer = keys };
    // println!("hi");
}

pub fn ModifierToKeys(byte: u8) -> Vec<Key> {
    let mut keys = vec![];
    if byte & 0x01 != 0 {
        keys.push(Key::LeftCtrl);
    }
    if byte & 0x02 != 0 {
        keys.push(Key::LeftShift);
    }
    if byte & 0x04 != 0 {
        keys.push(Key::LeftAlt);
    }
    if byte & 0x08 != 0 {
        keys.push(Key::LeftGui); //Windows key, Command on Mac
    }
    if byte & 0x10 != 0 {
        keys.push(Key::RightCtrl);
    }
    if byte & 0x20 != 0 {
        keys.push(Key::RightShift);
    }
    if byte & 0x40 != 0 {
        keys.push(Key::RightAlt);
    }
    if byte & 0x80 != 0 {
        keys.push(Key::RightGui);
    }
    return keys;
    
}

//Table is here: https://github.com/tmk/tmk_keyboard/wiki/USB%3A-HID-Usage-Table
pub fn ByteToKey(byte: u8) -> Key{
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