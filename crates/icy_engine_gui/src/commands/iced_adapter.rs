//! Adapter to convert Iced keyboard events to our Hotkey system
//!
//! This module bridges Iced's keyboard API with the command system.

use iced::keyboard::{self, Key, Modifiers as IcedModifiers};
use super::{Hotkey, KeyCode, Modifiers};

/// Convert Iced modifiers to our Modifiers
pub fn from_iced_modifiers(mods: IcedModifiers) -> Modifiers {
    Modifiers {
        ctrl: mods.control(),
        alt: mods.alt(),
        shift: mods.shift(),
        cmd: mods.command(),
    }
}

/// Convert Iced Key to our KeyCode
pub fn from_iced_key(key: &Key) -> Option<KeyCode> {
    match key {
        Key::Character(s) => {
            let s = s.to_lowercase();
            let c = s.chars().next()?;
            KeyCode::from_str(&c.to_string())
        }
        Key::Named(named) => from_iced_named(*named),
        Key::Unidentified => None,
    }
}

/// Convert Iced Named key to our KeyCode
fn from_iced_named(named: keyboard::key::Named) -> Option<KeyCode> {
    use keyboard::key::Named::*;
    
    Some(match named {
        Escape => KeyCode::Escape,
        Tab => KeyCode::Tab,
        Space => KeyCode::Space,
        Backspace => KeyCode::Backspace,
        Enter => KeyCode::Enter,
        Delete => KeyCode::Delete,
        Insert => KeyCode::Insert,
        Home => KeyCode::Home,
        End => KeyCode::End,
        PageUp => KeyCode::PageUp,
        PageDown => KeyCode::PageDown,
        ArrowUp => KeyCode::ArrowUp,
        ArrowDown => KeyCode::ArrowDown,
        ArrowLeft => KeyCode::ArrowLeft,
        ArrowRight => KeyCode::ArrowRight,
        F1 => KeyCode::F1,
        F2 => KeyCode::F2,
        F3 => KeyCode::F3,
        F4 => KeyCode::F4,
        F5 => KeyCode::F5,
        F6 => KeyCode::F6,
        F7 => KeyCode::F7,
        F8 => KeyCode::F8,
        F9 => KeyCode::F9,
        F10 => KeyCode::F10,
        F11 => KeyCode::F11,
        F12 => KeyCode::F12,
        _ => return None,
    })
}

/// Create a Hotkey from an Iced keyboard event
///
/// Returns None if the key cannot be mapped to a Hotkey
pub fn hotkey_from_iced(key: &Key, modifiers: IcedModifiers) -> Option<Hotkey> {
    let key_code = from_iced_key(key)?;
    let mods = from_iced_modifiers(modifiers);
    Some(Hotkey::new(key_code, mods))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_iced_modifiers() {
        // Empty modifiers
        let mods = Modifiers::default();
        assert!(!mods.ctrl);
        assert!(!mods.alt);
        assert!(!mods.shift);
        assert!(!mods.cmd);
    }

    #[test]
    fn test_from_iced_character_key() {
        let key = Key::Character("c".into());
        let keycode = from_iced_key(&key);
        assert_eq!(keycode, Some(KeyCode::C));
    }

    #[test]
    fn test_from_iced_named_key() {
        let key = Key::Named(keyboard::key::Named::F11);
        let keycode = from_iced_key(&key);
        assert_eq!(keycode, Some(KeyCode::F11));
    }

    #[test]
    fn test_hotkey_from_iced() {
        // Simulate Ctrl+C
        let key = Key::Character("c".into());
        let mods = IcedModifiers::CTRL;
        
        let hotkey = hotkey_from_iced(&key, mods).unwrap();
        assert_eq!(hotkey.key, KeyCode::C);
        assert!(hotkey.modifiers.ctrl);
        assert!(!hotkey.modifiers.shift);
    }
}
