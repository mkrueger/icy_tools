//! Kitty keyboard protocol key encoding.
//!
//! <https://sw.kovidgoyal.net/kitty/keyboard-protocol/>
//!
//! Games and full-screen applications need key *release* events, which the
//! legacy byte stream cannot express. Once an application pushes progressive
//! enhancement flags, keys are reported as `CSI unicode-key ; modifiers:event u`
//! (or a `~`/letter final for keys that have one).

use icy_ui::keyboard::{self, key::Code, key::Named, Location};

pub const DISAMBIGUATE: u8 = 0b1;
pub const REPORT_EVENT_TYPES: u8 = 0b10;
pub const REPORT_ALTERNATE_KEYS: u8 = 0b100;
pub const REPORT_ALL_KEYS: u8 = 0b1000;
pub const REPORT_ASSOCIATED_TEXT: u8 = 0b1_0000;

/// Flags this terminal implements. Alternate-key reporting is not supported.
pub const SUPPORTED_FLAGS: u8 = DISAMBIGUATE | REPORT_EVENT_TYPES | REPORT_ALL_KEYS | REPORT_ASSOCIATED_TEXT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyId {
    /// `CSI <number> u`
    Unicode(u32),
    /// `CSI <number> ~`
    Tilde(u32),
    /// `CSI 1 ; <mods> <final>`
    Final(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    Press,
    Repeat,
    Release,
}

impl KeyEventKind {
    fn code(self) -> u8 {
        match self {
            KeyEventKind::Press => 1,
            KeyEventKind::Repeat => 2,
            KeyEventKind::Release => 3,
        }
    }
}

fn modifier_bits(modifiers: keyboard::Modifiers) -> u8 {
    let mut bits = 0;
    if modifiers.shift() {
        bits |= 0b1;
    }
    if modifiers.alt() {
        bits |= 0b10;
    }
    if modifiers.control() {
        bits |= 0b100;
    }
    if modifiers.logo() {
        bits |= 0b1000;
    }
    bits
}

/// Keypad keys carry their own codepoints so applications can tell them apart
/// from the main block.
fn keypad_key(code: Code) -> Option<u32> {
    Some(match code {
        Code::Numpad0 => 57399,
        Code::Numpad1 => 57400,
        Code::Numpad2 => 57401,
        Code::Numpad3 => 57402,
        Code::Numpad4 => 57403,
        Code::Numpad5 => 57404,
        Code::Numpad6 => 57405,
        Code::Numpad7 => 57406,
        Code::Numpad8 => 57407,
        Code::Numpad9 => 57408,
        Code::NumpadDecimal => 57409,
        Code::NumpadDivide => 57410,
        Code::NumpadMultiply => 57411,
        Code::NumpadSubtract => 57412,
        Code::NumpadAdd => 57413,
        Code::NumpadEnter => 57414,
        Code::NumpadEqual => 57415,
        Code::NumpadComma => 57416,
        _ => return None,
    })
}

/// The navigation function of a keypad key, used when NumLock is off.
fn keypad_named(named: Named) -> Option<u32> {
    Some(match named {
        Named::ArrowLeft => 57417,
        Named::ArrowRight => 57418,
        Named::ArrowUp => 57419,
        Named::ArrowDown => 57420,
        Named::PageUp => 57421,
        Named::PageDown => 57422,
        Named::Home => 57423,
        Named::End => 57424,
        Named::Insert => 57425,
        Named::Delete => 57426,
        _ => return None,
    })
}

fn named_key(named: Named, location: Location) -> Option<KeyId> {
    let right = location == Location::Right;
    Some(match named {
        Named::Escape => KeyId::Unicode(27),
        Named::Enter => KeyId::Unicode(13),
        Named::Tab => KeyId::Unicode(9),
        Named::Backspace => KeyId::Unicode(127),
        Named::Space => KeyId::Unicode(32),

        Named::Insert => KeyId::Tilde(2),
        Named::Delete => KeyId::Tilde(3),
        Named::PageUp => KeyId::Tilde(5),
        Named::PageDown => KeyId::Tilde(6),

        Named::ArrowUp => KeyId::Final(b'A'),
        Named::ArrowDown => KeyId::Final(b'B'),
        Named::ArrowRight => KeyId::Final(b'C'),
        Named::ArrowLeft => KeyId::Final(b'D'),
        Named::End => KeyId::Final(b'F'),
        Named::Home => KeyId::Final(b'H'),

        Named::F1 => KeyId::Final(b'P'),
        Named::F2 => KeyId::Final(b'Q'),
        Named::F3 => KeyId::Tilde(13),
        Named::F4 => KeyId::Final(b'S'),
        Named::F5 => KeyId::Tilde(15),
        Named::F6 => KeyId::Tilde(17),
        Named::F7 => KeyId::Tilde(18),
        Named::F8 => KeyId::Tilde(19),
        Named::F9 => KeyId::Tilde(20),
        Named::F10 => KeyId::Tilde(21),
        Named::F11 => KeyId::Tilde(23),
        Named::F12 => KeyId::Tilde(24),

        Named::CapsLock => KeyId::Unicode(57358),
        Named::ScrollLock => KeyId::Unicode(57359),
        Named::NumLock => KeyId::Unicode(57360),
        Named::PrintScreen => KeyId::Unicode(57361),
        Named::Pause => KeyId::Unicode(57362),
        Named::ContextMenu => KeyId::Unicode(57363),

        Named::Shift => KeyId::Unicode(if right { 57447 } else { 57441 }),
        Named::Control => KeyId::Unicode(if right { 57448 } else { 57442 }),
        Named::Alt => KeyId::Unicode(if right { 57449 } else { 57443 }),
        Named::Super => KeyId::Unicode(if right { 57450 } else { 57444 }),
        Named::Hyper => KeyId::Unicode(if right { 57451 } else { 57445 }),
        Named::Meta => KeyId::Unicode(if right { 57452 } else { 57446 }),
        _ => return None,
    })
}

fn is_modifier_key(named: Named) -> bool {
    matches!(
        named,
        Named::Shift | Named::Control | Named::Alt | Named::Super | Named::Hyper | Named::Meta | Named::CapsLock | Named::NumLock | Named::ScrollLock
    )
}

/// Resolves the key identity, preferring the physical keypad codes so that a
/// keypad key is never confused with its main-block twin.
fn key_id(key: &keyboard::Key, physical: &keyboard::key::Physical, location: Location) -> Option<KeyId> {
    if location == Location::Numpad {
        if let keyboard::key::Physical::Code(code) = physical {
            if let Some(number) = keypad_key(*code) {
                return Some(KeyId::Unicode(number));
            }
        }
        if let keyboard::Key::Named(named) = key {
            if let Some(number) = keypad_named(*named) {
                return Some(KeyId::Unicode(number));
            }
        }
    }
    match key {
        keyboard::Key::Named(named) => named_key(*named, location),
        keyboard::Key::Character(text) => {
            let character = text.chars().next()?;
            // The protocol keys off the unshifted form.
            let lowered = character.to_lowercase().next().unwrap_or(character);
            Some(KeyId::Unicode(lowered as u32))
        }
        keyboard::Key::Unidentified => None,
    }
}

/// Encodes one key event, or `None` when the caller should fall back to the
/// legacy byte encoding.
pub fn encode_key_event(
    flags: u8,
    key: &keyboard::Key,
    physical: &keyboard::key::Physical,
    location: Location,
    modifiers: keyboard::Modifiers,
    text: Option<&str>,
    kind: KeyEventKind,
) -> Option<Vec<u8>> {
    if flags == 0 {
        return None;
    }
    let report_events = flags & REPORT_EVENT_TYPES != 0;
    if kind != KeyEventKind::Press && !report_events {
        return None;
    }

    let all_keys = flags & REPORT_ALL_KEYS != 0;
    if let keyboard::Key::Named(named) = key {
        // Modifier keys only report themselves when all keys are requested.
        if is_modifier_key(*named) && !all_keys {
            return None;
        }
    }

    let id = key_id(key, physical, location)?;
    let bits = modifier_bits(modifiers);

    // Without `report all keys`, plain text still travels as text.
    if !all_keys && matches!(id, KeyId::Unicode(_)) && matches!(key, keyboard::Key::Character(_)) && bits & 0b1110 == 0 {
        return None;
    }

    let mut sequence = String::from("\x1b[");
    let number = match id {
        KeyId::Unicode(number) => number,
        KeyId::Tilde(number) => number,
        KeyId::Final(_) => 1,
    };

    let event = kind.code();
    let associated = if flags & REPORT_ASSOCIATED_TEXT != 0 && kind != KeyEventKind::Release {
        text.filter(|text| !text.is_empty() && !text.chars().any(|c| c.is_control()))
    } else {
        None
    };
    let needs_modifiers = bits != 0 || (report_events && event != 1) || associated.is_some();

    sequence.push_str(&number.to_string());
    if needs_modifiers {
        sequence.push(';');
        sequence.push_str(&(bits + 1).to_string());
        if report_events && event != 1 {
            sequence.push(':');
            sequence.push_str(&event.to_string());
        }
        if let Some(associated) = associated {
            sequence.push(';');
            let codepoints: Vec<String> = associated.chars().map(|c| (c as u32).to_string()).collect();
            sequence.push_str(&codepoints.join(":"));
        }
    }

    match id {
        KeyId::Unicode(_) => sequence.push('u'),
        KeyId::Tilde(_) => sequence.push('~'),
        KeyId::Final(final_byte) => sequence.push(final_byte as char),
    }
    Some(sequence.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_ui::keyboard::{key::NativeCode, key::Physical, Key, Modifiers};

    const ALL: u8 = DISAMBIGUATE | REPORT_EVENT_TYPES | REPORT_ALL_KEYS;

    fn unknown() -> Physical {
        Physical::Unidentified(NativeCode::Unidentified)
    }

    fn encode(flags: u8, key: Key, physical: Physical, location: Location, modifiers: Modifiers, kind: KeyEventKind) -> Option<String> {
        encode_key_event(flags, &key, &physical, location, modifiers, None, kind).map(|bytes| String::from_utf8(bytes).unwrap())
    }

    #[test]
    fn encodes_press_repeat_and_release() {
        let key = || Key::Character("a".into());
        assert_eq!(
            encode(ALL, key(), unknown(), Location::Standard, Modifiers::empty(), KeyEventKind::Press),
            Some("\x1b[97u".to_string())
        );
        assert_eq!(
            encode(ALL, key(), unknown(), Location::Standard, Modifiers::empty(), KeyEventKind::Repeat),
            Some("\x1b[97;1:2u".to_string())
        );
        assert_eq!(
            encode(ALL, key(), unknown(), Location::Standard, Modifiers::empty(), KeyEventKind::Release),
            Some("\x1b[97;1:3u".to_string())
        );
    }

    #[test]
    fn releases_are_silent_without_event_reporting() {
        let flags = DISAMBIGUATE | REPORT_ALL_KEYS;
        assert_eq!(
            encode(
                flags,
                Key::Character("a".into()),
                unknown(),
                Location::Standard,
                Modifiers::empty(),
                KeyEventKind::Release
            ),
            None
        );
    }

    #[test]
    fn plain_text_stays_legacy_until_all_keys_is_requested() {
        // Disambiguate alone must not turn ordinary typing into escape codes.
        assert_eq!(
            encode(
                DISAMBIGUATE,
                Key::Character("a".into()),
                unknown(),
                Location::Standard,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            None
        );
        // Ctrl is ambiguous in the legacy encoding, so it is always reported.
        assert_eq!(
            encode(
                DISAMBIGUATE,
                Key::Character("a".into()),
                unknown(),
                Location::Standard,
                Modifiers::CTRL,
                KeyEventKind::Press
            ),
            Some("\x1b[97;5u".to_string())
        );
    }

    #[test]
    fn encodes_arrows_with_legacy_finals() {
        assert_eq!(
            encode(
                ALL,
                Key::Named(Named::ArrowUp),
                unknown(),
                Location::Standard,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            Some("\x1b[1A".to_string())
        );
        assert_eq!(
            encode(
                ALL,
                Key::Named(Named::ArrowUp),
                unknown(),
                Location::Standard,
                Modifiers::empty(),
                KeyEventKind::Release
            ),
            Some("\x1b[1;1:3A".to_string())
        );
        assert_eq!(
            encode(
                ALL,
                Key::Named(Named::ArrowLeft),
                unknown(),
                Location::Standard,
                Modifiers::SHIFT,
                KeyEventKind::Press
            ),
            Some("\x1b[1;2D".to_string())
        );
    }

    #[test]
    fn encodes_keypad_separately_from_the_main_block() {
        // The numpad plus must not look like a typed '+'.
        assert_eq!(
            encode(
                ALL,
                Key::Character("+".into()),
                Physical::Code(Code::NumpadAdd),
                Location::Numpad,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            Some("\x1b[57413u".to_string())
        );
        assert_eq!(
            encode(
                ALL,
                Key::Character("5".into()),
                Physical::Code(Code::Numpad5),
                Location::Numpad,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            Some("\x1b[57404u".to_string())
        );
        // NumLock off: the key reports its navigation function.
        assert_eq!(
            encode(
                ALL,
                Key::Named(Named::Home),
                Physical::Unidentified(NativeCode::Unidentified),
                Location::Numpad,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            Some("\x1b[57423u".to_string())
        );
    }

    #[test]
    fn encodes_function_and_editing_keys() {
        assert_eq!(
            encode(
                ALL,
                Key::Named(Named::F1),
                unknown(),
                Location::Standard,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            Some("\x1b[1P".to_string())
        );
        assert_eq!(
            encode(
                ALL,
                Key::Named(Named::F5),
                unknown(),
                Location::Standard,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            Some("\x1b[15~".to_string())
        );
        assert_eq!(
            encode(
                ALL,
                Key::Named(Named::Delete),
                unknown(),
                Location::Standard,
                Modifiers::empty(),
                KeyEventKind::Release
            ),
            Some("\x1b[3;1:3~".to_string())
        );
        assert_eq!(
            encode(
                ALL,
                Key::Named(Named::Escape),
                unknown(),
                Location::Standard,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            Some("\x1b[27u".to_string())
        );
    }

    #[test]
    fn modifier_keys_need_report_all_keys() {
        assert_eq!(
            encode(
                DISAMBIGUATE | REPORT_EVENT_TYPES,
                Key::Named(Named::Shift),
                unknown(),
                Location::Left,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            None
        );
        assert_eq!(
            encode(
                ALL,
                Key::Named(Named::Shift),
                unknown(),
                Location::Left,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            Some("\x1b[57441u".to_string())
        );
        assert_eq!(
            encode(
                ALL,
                Key::Named(Named::Control),
                unknown(),
                Location::Right,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            Some("\x1b[57448u".to_string())
        );
    }

    #[test]
    fn appends_associated_text_when_requested() {
        let flags = ALL | REPORT_ASSOCIATED_TEXT;
        let bytes = encode_key_event(
            flags,
            &Key::Character("a".into()),
            &unknown(),
            Location::Standard,
            Modifiers::empty(),
            Some("a"),
            KeyEventKind::Press,
        )
        .unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), "\x1b[97;1;97u");
    }

    #[test]
    fn disabled_protocol_defers_to_the_legacy_encoding() {
        assert_eq!(
            encode(
                0,
                Key::Named(Named::ArrowUp),
                unknown(),
                Location::Standard,
                Modifiers::empty(),
                KeyEventKind::Press
            ),
            None
        );
    }
}
