//! CTerm physical key reports (`CSI = 1 h`), as SyncTERM sends them: a key is reported by its
//! evdev code as `CSI = Pk K` when pressed and `CSI = Pk k` when released. Doors use this to
//! see held keys, e.g. a game reading the arrow keys like a controller.

use std::collections::BTreeSet;

use eframe::egui;

const KEY_LEFTCTRL: u16 = 29;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_LEFTALT: u16 = 56;

/// Remembers which keys were reported as pressed, so each edge is reported once and a release
/// is only reported for a key whose press was.
#[derive(Default)]
pub struct PhysicalKeys {
    held: BTreeSet<u16>,
}

impl PhysicalKeys {
    /// Reports the key edges in `events`. Key repeats are not edges and are left out.
    /// `modifiers` is the modifier state after the events; egui has no events for modifier keys.
    pub fn encode(&mut self, events: &[egui::Event], modifiers: egui::Modifiers) -> Vec<u8> {
        let mut output = Vec::new();
        for event in events {
            if let egui::Event::Key {
                key,
                physical_key,
                pressed,
                repeat: false,
                modifiers,
            } = event
            {
                self.report_modifiers(*modifiers, &mut output);
                if let Some(code) = evdev_code(physical_key.unwrap_or(*key)) {
                    self.report(code, *pressed, &mut output);
                }
            }
        }
        self.report_modifiers(modifiers, &mut output);
        output
    }

    /// Reports every held key as released, for when the terminal stops getting key events.
    pub fn release_all(&mut self) -> Vec<u8> {
        let mut output = Vec::new();
        for code in std::mem::take(&mut self.held) {
            output.extend_from_slice(format!("\x1b[={code}k").as_bytes());
        }
        output
    }

    /// Forgets the held keys without reporting them, as CTerm does when the reports are turned off.
    pub fn clear(&mut self) {
        self.held.clear();
    }

    fn report_modifiers(&mut self, modifiers: egui::Modifiers, output: &mut Vec<u8>) {
        self.report(KEY_LEFTSHIFT, modifiers.shift, output);
        self.report(KEY_LEFTCTRL, modifiers.ctrl, output);
        self.report(KEY_LEFTALT, modifiers.alt, output);
    }

    fn report(&mut self, code: u16, pressed: bool, output: &mut Vec<u8>) {
        let changed = if pressed { self.held.insert(code) } else { self.held.remove(&code) };
        if changed {
            output.extend_from_slice(format!("\x1b[={code}{}", if pressed { 'K' } else { 'k' }).as_bytes());
        }
    }
}

/// The evdev key code (`linux/input-event-codes.h`) for the key at this position on a US keyboard.
fn evdev_code(key: egui::Key) -> Option<u16> {
    use egui::Key as K;
    Some(match key {
        K::Escape => 1,
        K::Num1 => 2,
        K::Num2 => 3,
        K::Num3 => 4,
        K::Num4 => 5,
        K::Num5 => 6,
        K::Num6 => 7,
        K::Num7 => 8,
        K::Num8 => 9,
        K::Num9 => 10,
        K::Num0 => 11,
        K::Minus => 12,
        K::Equals => 13,
        K::Backspace => 14,
        K::Tab => 15,
        K::Q => 16,
        K::W => 17,
        K::E => 18,
        K::R => 19,
        K::T => 20,
        K::Y => 21,
        K::U => 22,
        K::I => 23,
        K::O => 24,
        K::P => 25,
        K::OpenBracket => 26,
        K::CloseBracket => 27,
        K::Enter => 28,
        K::A => 30,
        K::S => 31,
        K::D => 32,
        K::F => 33,
        K::G => 34,
        K::H => 35,
        K::J => 36,
        K::K => 37,
        K::L => 38,
        K::Semicolon => 39,
        K::Quote => 40,
        K::Backtick => 41,
        K::Backslash => 43,
        K::Z => 44,
        K::X => 45,
        K::C => 46,
        K::V => 47,
        K::B => 48,
        K::N => 49,
        K::M => 50,
        K::Comma => 51,
        K::Period => 52,
        K::Slash => 53,
        K::Space => 57,
        K::F1 => 59,
        K::F2 => 60,
        K::F3 => 61,
        K::F4 => 62,
        K::F5 => 63,
        K::F6 => 64,
        K::F7 => 65,
        K::F8 => 66,
        K::F9 => 67,
        K::F10 => 68,
        K::F11 => 87,
        K::F12 => 88,
        K::Home => 102,
        K::ArrowUp => 103,
        K::PageUp => 104,
        K::ArrowLeft => 105,
        K::ArrowRight => 106,
        K::End => 107,
        K::ArrowDown => 108,
        K::PageDown => 109,
        K::Insert => 110,
        K::Delete => 111,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: egui::Key, pressed: bool, repeat: bool, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat,
            modifiers,
        }
    }

    #[test]
    fn held_keys_report_one_press_and_one_release() {
        let mut keys = PhysicalKeys::default();
        let none = egui::Modifiers::NONE;
        let events = [
            key(egui::Key::ArrowRight, true, false, none),
            key(egui::Key::ArrowRight, true, true, none),
            key(egui::Key::X, true, false, none),
            key(egui::Key::X, false, false, none),
            key(egui::Key::ArrowRight, true, true, none),
        ];
        assert_eq!(keys.encode(&events, none), b"\x1b[=106K\x1b[=45K\x1b[=45k");
        assert_eq!(keys.encode(&[key(egui::Key::ArrowRight, false, false, none)], none), b"\x1b[=106k");
    }

    #[test]
    fn the_physical_key_wins_over_the_layout() {
        let mut keys = PhysicalKeys::default();
        let event = egui::Event::Key {
            key: egui::Key::Z,
            physical_key: Some(egui::Key::Y),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        assert_eq!(keys.encode(&[event], egui::Modifiers::NONE), b"\x1b[=21K");
    }

    #[test]
    fn modifiers_are_reported_as_their_own_keys() {
        let mut keys = PhysicalKeys::default();
        let ctrl = egui::Modifiers::CTRL;
        assert_eq!(keys.encode(&[key(egui::Key::Q, true, false, ctrl)], ctrl), b"\x1b[=29K\x1b[=16K");
        assert_eq!(keys.encode(&[], egui::Modifiers::NONE), b"\x1b[=29k");
    }

    #[test]
    fn release_all_reports_and_forgets_held_keys() {
        let mut keys = PhysicalKeys::default();
        let none = egui::Modifiers::NONE;
        keys.encode(&[key(egui::Key::A, true, false, none), key(egui::Key::ArrowUp, true, false, none)], none);
        assert_eq!(keys.release_all(), b"\x1b[=30k\x1b[=103k");
        assert!(keys.release_all().is_empty());
        keys.encode(&[key(egui::Key::A, true, false, none)], none);
        keys.clear();
        assert!(keys.encode(&[key(egui::Key::A, false, false, none)], none).is_empty());
    }
}
