//! Adapter to convert Iced keyboard and mouse events to our command system
//!
//! This module bridges Iced's keyboard and mouse API with the command system.

use super::{Hotkey, KeyCode, Modifiers, MouseBinding, MouseButton};
use iced::keyboard::{self, Key, Modifiers as IcedModifiers};

/// Convert Iced modifiers to our Modifiers
///
/// Note: We use `logo()` instead of `command()` because Iced's `command()`
/// returns true for Ctrl on Linux/Windows (for cross-platform shortcuts).
/// We handle platform differences via separate hotkey/hotkey_mac definitions,
/// so we only want the actual macOS Command key (Super/Logo key).
pub fn from_iced_modifiers(mods: IcedModifiers) -> Modifiers {
    Modifiers {
        ctrl: mods.control(),
        alt: mods.alt(),
        shift: mods.shift(),
        cmd: mods.logo(), // Use logo() - only true for macOS Cmd key
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

/// Convert Iced mouse button to our MouseButton
pub fn from_iced_mouse_button(button: iced::mouse::Button) -> MouseButton {
    match button {
        iced::mouse::Button::Left => MouseButton::Left,
        iced::mouse::Button::Right => MouseButton::Right,
        iced::mouse::Button::Middle => MouseButton::Middle,
        iced::mouse::Button::Back => MouseButton::Back,
        iced::mouse::Button::Forward => MouseButton::Forward,
        iced::mouse::Button::Other(n) => MouseButton::Other(n),
    }
}

/// Create a MouseBinding from an Iced mouse button event
pub fn mouse_binding_from_iced(button: iced::mouse::Button, modifiers: IcedModifiers) -> MouseBinding {
    let btn = from_iced_mouse_button(button);
    let mods = from_iced_modifiers(modifiers);
    MouseBinding::new(btn, mods)
}

/// Trait for types that can be converted to input bindings for command matching
pub trait IntoHotkey {
    fn into_hotkey(&self) -> Option<Hotkey>;
    fn into_mouse_binding(&self) -> Option<MouseBinding> {
        None
    }
}

impl IntoHotkey for Hotkey {
    fn into_hotkey(&self) -> Option<Hotkey> {
        Some(*self)
    }
}

impl IntoHotkey for &Hotkey {
    fn into_hotkey(&self) -> Option<Hotkey> {
        Some(**self)
    }
}

impl IntoHotkey for MouseBinding {
    fn into_hotkey(&self) -> Option<Hotkey> {
        None
    }
    fn into_mouse_binding(&self) -> Option<MouseBinding> {
        Some(*self)
    }
}

impl IntoHotkey for iced::Event {
    fn into_hotkey(&self) -> Option<Hotkey> {
        if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) = self {
            hotkey_from_iced(key, *modifiers)
        } else {
            None
        }
    }

    fn into_mouse_binding(&self) -> Option<MouseBinding> {
        if let iced::Event::Mouse(iced::mouse::Event::ButtonPressed { button, modifiers }) = self {
            Some(MouseBinding::new(from_iced_mouse_button(*button), from_iced_modifiers(*modifiers)))
        } else {
            None
        }
    }
}

impl IntoHotkey for &iced::Event {
    fn into_hotkey(&self) -> Option<Hotkey> {
        (*self).into_hotkey()
    }
    fn into_mouse_binding(&self) -> Option<MouseBinding> {
        (*self).into_mouse_binding()
    }
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
