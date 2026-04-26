// keys.rs -- Keyboard input mapping for Geometry OS

use minifb::Key;

pub fn key_to_ascii(key: Key) -> Option<u8> {
    match key {
        Key::A => Some(b'A'),
        Key::B => Some(b'B'),
        Key::C => Some(b'C'),
        Key::D => Some(b'D'),
        Key::E => Some(b'E'),
        Key::F => Some(b'F'),
        Key::G => Some(b'G'),
        Key::H => Some(b'H'),
        Key::I => Some(b'I'),
        Key::J => Some(b'J'),
        Key::K => Some(b'K'),
        Key::L => Some(b'L'),
        Key::M => Some(b'M'),
        Key::N => Some(b'N'),
        Key::O => Some(b'O'),
        Key::P => Some(b'P'),
        Key::Q => Some(b'Q'),
        Key::R => Some(b'R'),
        Key::S => Some(b'S'),
        Key::T => Some(b'T'),
        Key::U => Some(b'U'),
        Key::V => Some(b'V'),
        Key::W => Some(b'W'),
        Key::X => Some(b'X'),
        Key::Y => Some(b'Y'),
        Key::Z => Some(b'Z'),
        Key::Key0 => Some(b'0'),
        Key::Key1 => Some(b'1'),
        Key::Key2 => Some(b'2'),
        Key::Key3 => Some(b'3'),
        Key::Key4 => Some(b'4'),
        Key::Key5 => Some(b'5'),
        Key::Key6 => Some(b'6'),
        Key::Key7 => Some(b'7'),
        Key::Key8 => Some(b'8'),
        Key::Key9 => Some(b'9'),
        Key::Space => Some(b' '),
        Key::Comma => Some(b','),
        Key::Period => Some(b'.'),
        Key::Slash => Some(b'/'),
        Key::Semicolon => Some(b';'),
        Key::Apostrophe => Some(b'\''),
        Key::Minus => Some(b'-'),
        Key::Equal => Some(b'='),
        Key::LeftBracket => Some(b'['),
        Key::RightBracket => Some(b']'),
        Key::Backslash => Some(b'\\'),
        // Special keys -- return their control character values
        Key::Enter => Some(0x0D),     // CR
        Key::Backspace => Some(0x08), // BS
        Key::Tab => Some(0x09),       // HT
        Key::Escape => Some(0x1B),    // ESC
        Key::Delete => Some(0x7F),    // DEL
        // Arrow keys -- use high codes (0x80+) that IKEY consumers can detect
        Key::Up => Some(0x80),       // extended: up arrow
        Key::Down => Some(0x81),     // extended: down arrow
        Key::Left => Some(0x82),     // extended: left arrow
        Key::Right => Some(0x83),    // extended: right arrow
        Key::Home => Some(0x84),     // extended: home
        Key::End => Some(0x85),      // extended: end
        Key::PageUp => Some(0x86),   // extended: page up
        Key::PageDown => Some(0x87), // extended: page down
        Key::Insert => Some(0x88),   // extended: insert
        _ => None,
    }
}

/// Map Ctrl+Shift+key combos to extended key codes (0x90+).
/// Only called when both Ctrl and Shift are held.
pub fn key_ctrl_shift(key: Key) -> Option<u8> {
    match key {
        Key::T => Some(0x90), // Ctrl+Shift+T = new tab
        Key::W => Some(0x91), // Ctrl+Shift+W = close tab
        Key::C => Some(0x96), // Ctrl+Shift+C = copy
        Key::V => Some(0x97), // Ctrl+Shift+V = paste
        _ => None,
    }
}

/// Map Ctrl+number combos to extended key codes (0x92-0x95).
/// Only called when Ctrl is held (no Shift).
pub fn key_ctrl_num(key: Key) -> Option<u8> {
    match key {
        Key::Key1 => Some(0x92), // Ctrl+1 = switch to tab 1
        Key::Key2 => Some(0x93), // Ctrl+2 = switch to tab 2
        Key::Key3 => Some(0x94), // Ctrl+3 = switch to tab 3
        Key::Key4 => Some(0x95), // Ctrl+4 = switch to tab 4
        _ => None,
    }
}

pub fn key_to_ascii_shifted(key: Key, shift: bool) -> Option<u8> {
    // Letters
    match key {
        Key::A => return Some(if shift { b'A' } else { b'a' }),
        Key::B => return Some(if shift { b'B' } else { b'b' }),
        Key::C => return Some(if shift { b'C' } else { b'c' }),
        Key::D => return Some(if shift { b'D' } else { b'd' }),
        Key::E => return Some(if shift { b'E' } else { b'e' }),
        Key::F => return Some(if shift { b'F' } else { b'f' }),
        Key::G => return Some(if shift { b'G' } else { b'g' }),
        Key::H => return Some(if shift { b'H' } else { b'h' }),
        Key::I => return Some(if shift { b'I' } else { b'i' }),
        Key::J => return Some(if shift { b'J' } else { b'j' }),
        Key::K => return Some(if shift { b'K' } else { b'k' }),
        Key::L => return Some(if shift { b'L' } else { b'l' }),
        Key::M => return Some(if shift { b'M' } else { b'm' }),
        Key::N => return Some(if shift { b'N' } else { b'n' }),
        Key::O => return Some(if shift { b'O' } else { b'o' }),
        Key::P => return Some(if shift { b'P' } else { b'p' }),
        Key::Q => return Some(if shift { b'Q' } else { b'q' }),
        Key::R => return Some(if shift { b'R' } else { b'r' }),
        Key::S => return Some(if shift { b'S' } else { b's' }),
        Key::T => return Some(if shift { b'T' } else { b't' }),
        Key::U => return Some(if shift { b'U' } else { b'u' }),
        Key::V => return Some(if shift { b'V' } else { b'v' }),
        Key::W => return Some(if shift { b'W' } else { b'w' }),
        Key::X => return Some(if shift { b'X' } else { b'x' }),
        Key::Y => return Some(if shift { b'Y' } else { b'y' }),
        Key::Z => return Some(if shift { b'Z' } else { b'z' }),
        _ => {}
    }

    // Numbers and symbols
    match key {
        Key::Key0 => Some(if shift { b')' } else { b'0' }),
        Key::Key1 => Some(if shift { b'!' } else { b'1' }),
        Key::Key2 => Some(if shift { b'@' } else { b'2' }),
        Key::Key3 => Some(if shift { b'#' } else { b'3' }),
        Key::Key4 => Some(if shift { b'$' } else { b'4' }),
        Key::Key5 => Some(if shift { b'%' } else { b'5' }),
        Key::Key6 => Some(if shift { b'^' } else { b'6' }),
        Key::Key7 => Some(if shift { b'&' } else { b'7' }),
        Key::Key8 => Some(if shift { b'*' } else { b'8' }),
        Key::Key9 => Some(if shift { b'(' } else { b'9' }),
        Key::Comma => Some(if shift { b'<' } else { b',' }),
        Key::Period => Some(if shift { b'>' } else { b'.' }),
        Key::Slash => Some(if shift { b'?' } else { b'/' }),
        Key::Semicolon => Some(if shift { b':' } else { b';' }),
        Key::Apostrophe => Some(if shift { b'"' } else { b'\'' }),
        Key::Minus => Some(if shift { b'_' } else { b'-' }),
        Key::Equal => Some(if shift { b'+' } else { b'=' }),
        Key::LeftBracket => Some(if shift { b'{' } else { b'[' }),
        Key::RightBracket => Some(if shift { b'}' } else { b']' }),
        Key::Backslash => Some(if shift { b'|' } else { b'\\' }),
        Key::Backquote => Some(if shift { b'~' } else { b'`' }),
        _ => None,
    }
}
