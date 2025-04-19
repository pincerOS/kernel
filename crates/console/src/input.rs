use display_client::proto;

use crate::editor;

pub fn remap_input(
    input: proto::InputEvent,
    modifiers: editor::Modifiers,
) -> Option<editor::KeyEvent> {
    use editor::{FuncKey, KeyEvent, Keypress};
    use proto::ScanCode;
    if input.kind == proto::InputEvent::KIND_KEY {
        let mode = input.data1;
        let scan = input.data2;

        let char = proto::SCANCODES.get(scan as usize).copied().flatten();
        let resolved = if modifiers.shift {
            proto::SCANCODES_SHIFTED
                .get(scan as usize)
                .copied()
                .flatten()
                .or(char)
        } else {
            char
        };

        let keypress = match ScanCode(scan) {
            ScanCode::DOWN => Keypress::Function(modifiers, FuncKey::Down),
            ScanCode::LEFT => Keypress::Function(modifiers, FuncKey::Left),
            ScanCode::RIGHT => Keypress::Function(modifiers, FuncKey::Right),
            ScanCode::UP => Keypress::Function(modifiers, FuncKey::Up),

            ScanCode::ENTER => Keypress::Function(modifiers, FuncKey::Enter),
            ScanCode::TAB => Keypress::Function(modifiers, FuncKey::Tab),

            ScanCode::BACKSPACE => Keypress::Function(modifiers, FuncKey::Backspace),
            ScanCode::DELETE => Keypress::Function(modifiers, FuncKey::Delete),
            ScanCode::END => Keypress::Function(modifiers, FuncKey::End),
            ScanCode::ESCAPE => Keypress::Function(modifiers, FuncKey::Escape),
            ScanCode::HOME => Keypress::Function(modifiers, FuncKey::Home),
            ScanCode::INSERT => Keypress::Function(modifiers, FuncKey::Insert),
            // ScanCode::MENU => Keypress::Function(modifiers, FuncKey::Menu),
            ScanCode::PAGE_DOWN => Keypress::Function(modifiers, FuncKey::PageDown),
            ScanCode::PAGE_UP => Keypress::Function(modifiers, FuncKey::PageUp),
            // ScanCode::LEFT_SHIFT => Keypress::Function(modifiers, FuncKey::LeftShift),
            // ScanCode::RIGHT_SHIFT => Keypress::Function(modifiers, FuncKey::RightShift),
            // ScanCode::LEFT_CTRL => Keypress::Function(modifiers, FuncKey::LeftCtrl),
            // ScanCode::RIGHT_CTRL => Keypress::Function(modifiers, FuncKey::RightCtrl),
            // ScanCode::LEFT_ALT => Keypress::Function(modifiers, FuncKey::LeftAlt),
            // ScanCode::RIGHT_ALT => Keypress::Function(modifiers, FuncKey::RightAlt),
            _ => {
                if let Some(key) = char {
                    Keypress::Char(modifiers, key, resolved.unwrap_or(key))
                } else {
                    return None;
                }
            }
        };

        let event = match mode {
            1 => KeyEvent::Press(keypress),
            2 => KeyEvent::Release(keypress),
            3 => KeyEvent::Repeat(keypress),
            _ => return None,
        };
        Some(event)
    } else {
        None
    }
}
