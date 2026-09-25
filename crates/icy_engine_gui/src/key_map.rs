use crate::terminal_keys::{self, Code, Key, NamedKey};
pub use crate::terminal_keys::{KeyWithModifiers, ANSI_KEY_MAP, ATARI_ST_KEY_MAP, ATASCII_KEY_MAP, C64_KEY_MAP, MODE7_KEY_MAP, VIDEOTERM_KEY_MAP};
use icy_ui::keyboard;

fn named_key(key: keyboard::key::Named) -> Option<NamedKey> {
    Some(match key {
        keyboard::key::Named::Escape => NamedKey::Escape,
        keyboard::key::Named::Home => NamedKey::Home,
        keyboard::key::Named::Insert => NamedKey::Insert,
        keyboard::key::Named::Backspace => NamedKey::Backspace,
        keyboard::key::Named::Enter => NamedKey::Enter,
        keyboard::key::Named::Tab => NamedKey::Tab,
        keyboard::key::Named::Delete => NamedKey::Delete,
        keyboard::key::Named::End => NamedKey::End,
        keyboard::key::Named::PageUp => NamedKey::PageUp,
        keyboard::key::Named::PageDown => NamedKey::PageDown,
        keyboard::key::Named::F1 => NamedKey::F1,
        keyboard::key::Named::F2 => NamedKey::F2,
        keyboard::key::Named::F3 => NamedKey::F3,
        keyboard::key::Named::F4 => NamedKey::F4,
        keyboard::key::Named::F5 => NamedKey::F5,
        keyboard::key::Named::F6 => NamedKey::F6,
        keyboard::key::Named::F7 => NamedKey::F7,
        keyboard::key::Named::F8 => NamedKey::F8,
        keyboard::key::Named::F9 => NamedKey::F9,
        keyboard::key::Named::F10 => NamedKey::F10,
        keyboard::key::Named::F11 => NamedKey::F11,
        keyboard::key::Named::F12 => NamedKey::F12,
        keyboard::key::Named::ArrowUp => NamedKey::ArrowUp,
        keyboard::key::Named::ArrowDown => NamedKey::ArrowDown,
        keyboard::key::Named::ArrowRight => NamedKey::ArrowRight,
        keyboard::key::Named::ArrowLeft => NamedKey::ArrowLeft,
        _ => return None,
    })
}

fn modifiers(value: keyboard::Modifiers) -> icy_engine::KeyModifiers {
    icy_engine::KeyModifiers {
        shift: value.shift(),
        ctrl: value.control() || value.command(),
        alt: value.alt(),
        meta: value.logo(),
    }
}

pub fn lookup_key(
    key: &keyboard::Key,
    physical: &keyboard::key::Physical,
    input_modifiers: keyboard::Modifiers,
    map: &[(KeyWithModifiers, &[u8])],
) -> Option<Vec<u8>> {
    let key = match key {
        keyboard::Key::Named(named) => named_key(*named).map(Key::Named).unwrap_or(Key::Unidentified),
        keyboard::Key::Character(text) => Key::Character(text.to_string()),
        _ => Key::Unidentified,
    };
    let physical = match physical {
        keyboard::key::Physical::Code(keyboard::key::Code::NumpadEnter) => Some(Code::NumpadEnter),
        keyboard::key::Physical::Code(keyboard::key::Code::Backquote) => Some(Code::Backquote),
        keyboard::key::Physical::Code(keyboard::key::Code::NumpadMultiply) => Some(Code::NumpadMultiply),
        _ => None,
    };
    terminal_keys::lookup_key(&key, &physical, modifiers(input_modifiers), map)
}

#[cfg(test)]
fn ansi_modified_function_key(key: keyboard::key::Named, input_modifiers: keyboard::Modifiers) -> Option<Vec<u8>> {
    terminal_keys::ansi_modified_function_key(named_key(key)?, modifiers(input_modifiers))
}

#[cfg(test)]
mod tests {
    use super::{ansi_modified_function_key, lookup_key, MODE7_KEY_MAP, VIDEOTERM_KEY_MAP};
    use icy_ui::keyboard::{
        key::{Named, NativeCode, Physical},
        Key, Modifiers,
    };

    #[test]
    fn encodes_xterm_function_key_modifiers() {
        assert_eq!(ansi_modified_function_key(Named::F1, Modifiers::SHIFT).unwrap(), b"\x1b[1;2P");
        assert_eq!(ansi_modified_function_key(Named::F5, Modifiers::ALT).unwrap(), b"\x1b[15;3~");
        assert_eq!(
            ansi_modified_function_key(Named::F12, Modifiers::CTRL | Modifiers::SHIFT).unwrap(),
            b"\x1b[24;6~"
        );
        assert_eq!(
            ansi_modified_function_key(Named::F2, Modifiers::ALT | Modifiers::CTRL | Modifiers::SHIFT).unwrap(),
            b"\x1b[1;8Q"
        );
    }

    #[test]
    fn leaves_unmodified_function_keys_to_static_maps() {
        assert!(ansi_modified_function_key(Named::F1, Modifiers::empty()).is_none());
    }

    #[test]
    fn maps_prestel_punctuation() {
        let physical = Physical::Unidentified(NativeCode::Unidentified);
        assert_eq!(
            lookup_key(&Key::Character("#".into()), &physical, Modifiers::empty(), VIDEOTERM_KEY_MAP),
            Some(b"_".to_vec())
        );
        assert_eq!(
            lookup_key(&Key::Character("_".into()), &physical, Modifiers::empty(), VIDEOTERM_KEY_MAP),
            Some(b"`".to_vec())
        );
        assert_eq!(
            lookup_key(&Key::Character("`".into()), &physical, Modifiers::empty(), VIDEOTERM_KEY_MAP),
            Some(b"#".to_vec())
        );
    }

    #[test]
    fn maps_prestel_editing_keys() {
        let physical = Physical::Unidentified(NativeCode::Unidentified);
        assert_eq!(
            lookup_key(&Key::Named(Named::Home), &physical, Modifiers::empty(), VIDEOTERM_KEY_MAP),
            Some(vec![0x1E])
        );
        assert_eq!(
            lookup_key(&Key::Character("l".into()), &physical, Modifiers::CTRL, VIDEOTERM_KEY_MAP),
            Some(vec![0x0C])
        );
        assert_eq!(
            lookup_key(&Key::Character("t".into()), &physical, Modifiers::CTRL, VIDEOTERM_KEY_MAP),
            Some(vec![0x18])
        );
    }

    #[test]
    fn passes_mode7_control_letters() {
        let physical = Physical::Unidentified(NativeCode::Unidentified);
        assert_eq!(
            lookup_key(&Key::Character("a".into()), &physical, Modifiers::CTRL, MODE7_KEY_MAP),
            Some(vec![0x01])
        );
        assert_eq!(
            lookup_key(&Key::Character("z".into()), &physical, Modifiers::CTRL, MODE7_KEY_MAP),
            Some(vec![0x1A])
        );
    }
}
