use eframe::egui;
use icy_engine::{BufferType, KeyModifiers};
use icy_engine_gui::terminal_keys::{
    lookup_key, Key, NamedKey, ANSI_KEY_MAP, ATARI_ST_KEY_MAP, ATASCII_KEY_MAP, C64_KEY_MAP, MODE7_KEY_MAP, VIDEOTERM_KEY_MAP,
};
use icy_net::telnet::TerminalEmulation;

fn text_bytes(text: &str, buffer_type: BufferType) -> Vec<u8> {
    if buffer_type == BufferType::Unicode {
        return text.as_bytes().to_vec();
    }
    text.chars()
        .map(|character| {
            if character.is_ascii() && matches!(buffer_type, BufferType::CP437 | BufferType::Unicode) {
                character as u8
            } else {
                let converted = buffer_type.convert_from_unicode(character);
                if buffer_type.convert_to_unicode(converted) == character {
                    u8::try_from(converted as u32).unwrap_or(b'?')
                } else {
                    b'?'
                }
            }
        })
        .collect()
}

fn mapped_key(key: egui::Key) -> Option<Key> {
    use egui::Key as EguiKey;
    let named = match key {
        EguiKey::Enter => NamedKey::Enter,
        EguiKey::Escape => NamedKey::Escape,
        EguiKey::Tab => NamedKey::Tab,
        EguiKey::Backspace => NamedKey::Backspace,
        EguiKey::Delete => NamedKey::Delete,
        EguiKey::Insert => NamedKey::Insert,
        EguiKey::Home => NamedKey::Home,
        EguiKey::End => NamedKey::End,
        EguiKey::PageUp => NamedKey::PageUp,
        EguiKey::PageDown => NamedKey::PageDown,
        EguiKey::ArrowUp => NamedKey::ArrowUp,
        EguiKey::ArrowDown => NamedKey::ArrowDown,
        EguiKey::ArrowLeft => NamedKey::ArrowLeft,
        EguiKey::ArrowRight => NamedKey::ArrowRight,
        EguiKey::F1 => NamedKey::F1,
        EguiKey::F2 => NamedKey::F2,
        EguiKey::F3 => NamedKey::F3,
        EguiKey::F4 => NamedKey::F4,
        EguiKey::F5 => NamedKey::F5,
        EguiKey::F6 => NamedKey::F6,
        EguiKey::F7 => NamedKey::F7,
        EguiKey::F8 => NamedKey::F8,
        EguiKey::F9 => NamedKey::F9,
        EguiKey::F10 => NamedKey::F10,
        EguiKey::F11 => NamedKey::F11,
        EguiKey::F12 => NamedKey::F12,
        EguiKey::OpenBracket => return Some(Key::Character("[".into())),
        EguiKey::CloseBracket => return Some(Key::Character("]".into())),
        EguiKey::Backslash => return Some(Key::Character("\\".into())),
        EguiKey::Minus => return Some(Key::Character("-".into())),
        _ => return (key.name().len() == 1).then(|| Key::Character(key.name().to_ascii_lowercase())),
    };
    Some(Key::Named(named))
}

#[cfg(test)]
pub fn encode_events(events: &[egui::Event], buffer_type: BufferType, bracketed_paste: bool) -> Vec<u8> {
    encode_terminal_events(events, buffer_type, bracketed_paste, TerminalEmulation::Ansi)
}

pub fn encode_terminal_events(events: &[egui::Event], buffer_type: BufferType, bracketed_paste: bool, terminal: TerminalEmulation) -> Vec<u8> {
    encode_protocol_events(events, buffer_type, bracketed_paste, terminal, 0)
}

fn kitty_key(key: egui::Key) -> Option<icy_engine_gui::kitty_protocol::KeyId> {
    use egui::Key as EguiKey;
    use icy_engine_gui::kitty_protocol::KeyId;
    Some(match key {
        EguiKey::Escape => KeyId::Unicode(27),
        EguiKey::Enter => KeyId::Unicode(13),
        EguiKey::Tab => KeyId::Unicode(9),
        EguiKey::Backspace => KeyId::Unicode(127),
        EguiKey::Space => KeyId::Unicode(32),
        EguiKey::Insert => KeyId::Tilde(2),
        EguiKey::Delete => KeyId::Tilde(3),
        EguiKey::PageUp => KeyId::Tilde(5),
        EguiKey::PageDown => KeyId::Tilde(6),
        EguiKey::ArrowUp => KeyId::Final(b'A'),
        EguiKey::ArrowDown => KeyId::Final(b'B'),
        EguiKey::ArrowRight => KeyId::Final(b'C'),
        EguiKey::ArrowLeft => KeyId::Final(b'D'),
        EguiKey::End => KeyId::Final(b'F'),
        EguiKey::Home => KeyId::Final(b'H'),
        EguiKey::F1 => KeyId::Final(b'P'),
        EguiKey::F2 => KeyId::Final(b'Q'),
        EguiKey::F3 => KeyId::Tilde(13),
        EguiKey::F4 => KeyId::Final(b'S'),
        EguiKey::F5 => KeyId::Tilde(15),
        EguiKey::F6 => KeyId::Tilde(17),
        EguiKey::F7 => KeyId::Tilde(18),
        EguiKey::F8 => KeyId::Tilde(19),
        EguiKey::F9 => KeyId::Tilde(20),
        EguiKey::F10 => KeyId::Tilde(21),
        EguiKey::F11 => KeyId::Tilde(23),
        EguiKey::F12 => KeyId::Tilde(24),
        _ => {
            let Key::Character(text) = mapped_key(key)? else { return None };
            KeyId::Unicode(text.chars().next()? as u32)
        }
    })
}

pub fn encode_protocol_events(events: &[egui::Event], buffer_type: BufferType, bracketed_paste: bool, terminal: TerminalEmulation, kitty_flags: u8) -> Vec<u8> {
    let map = match terminal {
        TerminalEmulation::PETscii => C64_KEY_MAP,
        TerminalEmulation::ATAscii => ATASCII_KEY_MAP,
        TerminalEmulation::ViewData => VIDEOTERM_KEY_MAP,
        TerminalEmulation::Mode7 => MODE7_KEY_MAP,
        TerminalEmulation::AtariST => ATARI_ST_KEY_MAP,
        _ => ANSI_KEY_MAP,
    };
    let encode_text = |text: &str| {
        let mut bytes = Vec::new();
        for character in text.chars() {
            if let Some(mapped) = lookup_key(&Key::Character(character.to_string()), &None, KeyModifiers::default(), map) {
                bytes.extend(mapped);
            } else {
                bytes.extend(text_bytes(&character.to_string(), buffer_type));
            }
        }
        bytes
    };
    let mut output = Vec::new();
    let clipboard_event = events
        .iter()
        .any(|event| matches!(event, egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)));
    let mut consumed_text = vec![false; events.len()];
    for (index, event) in events.iter().enumerate() {
        if consumed_text[index] {
            continue;
        }
        if let egui::Event::Key {
            key,
            pressed,
            repeat,
            modifiers,
            ..
        } = event
        {
            if kitty_flags != 0
                && !(modifiers.ctrl && modifiers.alt)
                && !modifiers.mac_cmd
                && !(clipboard_event && modifiers.command && matches!(key, egui::Key::C | egui::Key::X | egui::Key::V))
            {
                use icy_engine_gui::kitty_protocol::{self, KeyEventKind};
                let associated = if *pressed {
                    events
                        .iter()
                        .enumerate()
                        .skip(index + 1)
                        .take_while(|(_, event)| !matches!(event, egui::Event::Key { .. }))
                        .find_map(|(index, event)| {
                            if let egui::Event::Text(text) = event {
                                Some((index, text.as_str()))
                            } else {
                                None
                            }
                        })
                } else {
                    None
                };
                let kind = if !pressed {
                    KeyEventKind::Release
                } else if *repeat && kitty_flags & kitty_protocol::REPORT_EVENT_TYPES != 0 {
                    KeyEventKind::Repeat
                } else {
                    KeyEventKind::Press
                };
                let bits = modifiers.shift as u8 | ((modifiers.alt as u8) << 1) | ((modifiers.ctrl as u8) << 2);
                if let Some(bytes) = kitty_key(*key).and_then(|id| {
                    kitty_protocol::encode(
                        kitty_flags,
                        id,
                        bits,
                        associated.map(|(_, text)| text),
                        kind,
                        matches!(mapped_key(*key), Some(Key::Character(_))),
                        false,
                    )
                }) {
                    output.extend(bytes);
                    if let Some((index, _)) = associated {
                        consumed_text[index] = true;
                    }
                    continue;
                }
            }
        }
        match event {
            egui::Event::Text(text) | egui::Event::Ime(egui::ImeEvent::Commit(text)) => output.extend(encode_text(text)),
            egui::Event::Paste(text) if !text.is_empty() => {
                let text = text.replace("\r\n", "\n").replace('\r', "\n");
                if bracketed_paste {
                    output.extend_from_slice(b"\x1b[200~");
                }
                let enter = lookup_key(&Key::Named(NamedKey::Enter), &None, KeyModifiers::default(), map).unwrap_or_else(|| vec![b'\r']);
                for (index, line) in text.split('\n').enumerate() {
                    if index > 0 {
                        output.extend_from_slice(&enter);
                    }
                    output.extend(encode_text(line));
                }
                if bracketed_paste {
                    output.extend_from_slice(b"\x1b[201~");
                }
            }
            egui::Event::Copy | egui::Event::Cut => {
                let character = if matches!(event, egui::Event::Copy) { b'c' } else { b'x' };
                let encoded = icy_engine_gui::kitty_protocol::encode(
                    kitty_flags,
                    icy_engine_gui::kitty_protocol::KeyId::Unicode(character as u32),
                    4,
                    None,
                    icy_engine_gui::kitty_protocol::KeyEventKind::Press,
                    true,
                    false,
                );
                if let Some(encoded) = encoded {
                    output.extend(encoded);
                } else {
                    output.push(character - b'a' + 1);
                }
            }
            egui::Event::Key {
                key,
                physical_key,
                pressed: true,
                modifiers,
                ..
            } => {
                if modifiers.mac_cmd || (modifiers.ctrl && modifiers.alt) {
                    continue;
                }
                if clipboard_event && modifiers.command && matches!(key, egui::Key::C | egui::Key::X | egui::Key::V) {
                    continue;
                }
                let shifted_c64_key = terminal == TerminalEmulation::PETscii && modifiers.shift && !modifiers.ctrl && !modifiers.alt;
                let key = if shifted_c64_key { physical_key.unwrap_or(*key) } else { *key };
                let Some(key) = mapped_key(key) else {
                    continue;
                };
                if matches!(key, Key::Character(_)) && (!modifiers.ctrl || modifiers.alt) && !shifted_c64_key {
                    continue;
                }
                let modifiers = KeyModifiers {
                    shift: modifiers.shift,
                    ctrl: modifiers.ctrl,
                    alt: modifiers.alt,
                    meta: false,
                };
                if let Some(bytes) = lookup_key(&key, &None, modifiers, map) {
                    output.extend(bytes);
                    if shifted_c64_key && matches!(key, Key::Character(_)) {
                        if let Some((text_index, _)) = events
                            .iter()
                            .enumerate()
                            .skip(index + 1)
                            .take_while(|(_, event)| !matches!(event, egui::Event::Key { .. }))
                            .find(|(_, event)| matches!(event, egui::Event::Text(_)))
                        {
                            consumed_text[text_index] = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn encodes_text_once_and_special_keys_using_shared_maps() {
        let events = [
            key(egui::Key::A, egui::Modifiers::NONE),
            egui::Event::Text("a".into()),
            key(egui::Key::Enter, egui::Modifiers::NONE),
            key(egui::Key::ArrowUp, egui::Modifiers::NONE),
            key(egui::Key::Tab, egui::Modifiers::SHIFT),
            key(egui::Key::F1, egui::Modifiers::CTRL),
        ];
        assert_eq!(encode_events(&events, BufferType::CP437, false), b"a\r\x1b[A\x1b[Z\x1b[1;5P");
        assert_eq!(encode_events(&[key(egui::Key::C, egui::Modifiers::CTRL)], BufferType::CP437, false), [3]);
        assert_eq!(
            encode_events(&[key(egui::Key::C, egui::Modifiers::COMMAND), egui::Event::Copy], BufferType::CP437, false),
            [3]
        );
    }

    #[test]
    fn kitty_reports_key_lifecycle_without_duplicate_text() {
        use icy_engine_gui::kitty_protocol::*;
        let events = [
            key(egui::Key::A, egui::Modifiers::NONE),
            egui::Event::Text("a".into()),
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        assert_eq!(
            encode_protocol_events(
                &events,
                BufferType::Unicode,
                false,
                TerminalEmulation::Ansi,
                DISAMBIGUATE | REPORT_ALL_KEYS | REPORT_EVENT_TYPES
            ),
            b"\x1b[97u\x1b[97;1:3u"
        );
        assert_eq!(
            encode_protocol_events(&events, BufferType::Unicode, false, TerminalEmulation::Ansi, DISAMBIGUATE),
            b"a"
        );
        assert_eq!(
            encode_protocol_events(
                &events,
                BufferType::Unicode,
                false,
                TerminalEmulation::Ansi,
                REPORT_ALL_KEYS | REPORT_ASSOCIATED_TEXT
            ),
            b"\x1b[97;1;97u"
        );
    }

    #[test]
    fn altgr_and_unicode_are_not_truncated_or_sent_twice() {
        let events = [
            key(
                egui::Key::Q,
                egui::Modifiers {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
            ),
            egui::Event::Text("@\u{e4}\u{1f642}".into()),
        ];
        assert_eq!(encode_events(&events, BufferType::CP437, false), [b'@', 0x84, b'?']);
        assert_eq!(encode_events(&events, BufferType::Unicode, false), "@\u{e4}\u{1f642}".as_bytes());
    }

    #[test]
    fn paste_normalizes_newlines_and_honors_remote_mode() {
        let events = [egui::Event::Paste("one\r\ntwo\nthree".into())];
        assert_eq!(encode_events(&events, BufferType::CP437, false), b"one\rtwo\rthree");
        assert_eq!(encode_events(&events, BufferType::CP437, true), b"\x1b[200~one\rtwo\rthree\x1b[201~");
    }

    #[test]
    fn legacy_emulations_use_their_own_key_and_paste_maps() {
        let events = [key(egui::Key::Enter, egui::Modifiers::NONE), key(egui::Key::ArrowUp, egui::Modifiers::NONE)];
        assert_eq!(
            encode_terminal_events(&events, BufferType::Atascii, false, TerminalEmulation::ATAscii),
            [155, 27, 28]
        );
        assert_eq!(
            encode_terminal_events(&events, BufferType::Petscii, false, TerminalEmulation::PETscii),
            [13, 145]
        );
        assert_eq!(
            encode_terminal_events(&[egui::Event::Paste("A\nB".into())], BufferType::Atascii, false, TerminalEmulation::ATAscii),
            [65, 155, 66]
        );
        assert_eq!(
            encode_terminal_events(&[egui::Event::Text("#".into())], BufferType::Viewdata, false, TerminalEmulation::ViewData),
            b"_"
        );
    }

    #[test]
    fn ime_sends_only_committed_text() {
        let events = [
            egui::Event::Ime(egui::ImeEvent::Preedit("pending".into())),
            egui::Event::Ime(egui::ImeEvent::Commit("\u{65e5}".into())),
        ];
        assert_eq!(encode_events(&events, BufferType::Unicode, false), "\u{65e5}".as_bytes());
    }

    #[test]
    fn c64_shift_numbers_use_the_key_map_without_duplicate_text() {
        for (number, texts, expected) in [
            (egui::Key::Num1, ["!", "!"], b'!'),
            (egui::Key::Num2, ["@", "\""], b'"'),
            (egui::Key::Num3, ["#", "\u{a7}"], b'#'),
            (egui::Key::Num4, ["$", "$"], b'$'),
            (egui::Key::Num5, ["%", "%"], b'%'),
            (egui::Key::Num6, ["^", "&"], b'&'),
            (egui::Key::Num7, ["&", "/"], b'\''),
            (egui::Key::Num8, ["*", "("], b'('),
            (egui::Key::Num9, ["(", ")"], b')'),
        ] {
            for text in texts {
                for physical_key in [None, Some(number)] {
                    let press = egui::Event::Key {
                        key: if physical_key.is_some() {
                            egui::Key::from_name(text).unwrap_or(number)
                        } else {
                            number
                        },
                        physical_key,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::SHIFT,
                    };
                    let events = [press.clone(), egui::Event::Text(text.into())];
                    assert_eq!(
                        encode_terminal_events(&events, BufferType::Petscii, false, TerminalEmulation::PETscii),
                        [expected],
                        "{number:?}: {text:?}, {physical_key:?}"
                    );
                    assert_eq!(encode_events(&events, BufferType::Unicode, false), text.as_bytes());
                    assert_eq!(
                        encode_terminal_events(&[press], BufferType::Petscii, false, TerminalEmulation::PETscii),
                        [expected]
                    );
                }
            }
        }
    }

    #[test]
    fn c64_shift_mapping_preserves_text_fallback_and_key_lifecycle() {
        let mut repeated = key(egui::Key::Num2, egui::Modifiers::SHIFT);
        if let egui::Event::Key { repeat, .. } = &mut repeated {
            *repeat = true;
        }
        let mut released = key(egui::Key::Num2, egui::Modifiers::SHIFT);
        if let egui::Event::Key { pressed, .. } = &mut released {
            *pressed = false;
        }
        let events = [
            key(egui::Key::Num2, egui::Modifiers::SHIFT),
            egui::Event::Text("@".into()),
            repeated,
            egui::Event::Text("@".into()),
            released,
            key(egui::Key::Num2, egui::Modifiers::NONE),
            egui::Event::Text("2".into()),
            key(egui::Key::Num0, egui::Modifiers::SHIFT),
            egui::Event::Text(")".into()),
            key(egui::Key::A, egui::Modifiers::SHIFT),
            egui::Event::Text("A".into()),
            key(
                egui::Key::Num2,
                egui::Modifiers {
                    shift: true,
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
            ),
            egui::Event::Text("@".into()),
            key(
                egui::Key::Num2,
                egui::Modifiers {
                    shift: true,
                    alt: true,
                    ..Default::default()
                },
            ),
            egui::Event::Text("@".into()),
            egui::Event::Paste("@".into()),
            egui::Event::Ime(egui::ImeEvent::Commit("@".into())),
        ];
        let mut expected = b"\"\"2)".to_vec();
        expected.extend(text_bytes("A@@@@", BufferType::Petscii));
        assert_eq!(
            encode_terminal_events(&events, BufferType::Petscii, false, TerminalEmulation::PETscii),
            expected
        );
    }
}

pub fn encode_mouse_events(input: &egui::InputState, state: &icy_engine::MouseState, render: &icy_engine_gui::terminal::RenderInfo) -> Vec<u8> {
    use icy_engine::{ExtMouseMode, MouseButton, MouseEvent, MouseEventType, MouseMode, Position};
    if !state.mouse_tracking_enabled || input.modifiers.shift {
        return Vec::new();
    }
    let mut output = Vec::new();
    for event in &input.events {
        let (position, kind, button, modifiers) = match event {
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } => (
                *pos,
                if *pressed { MouseEventType::Press } else { MouseEventType::Release },
                match button {
                    egui::PointerButton::Primary => MouseButton::Left,
                    egui::PointerButton::Secondary => MouseButton::Right,
                    egui::PointerButton::Middle => MouseButton::Middle,
                    _ => continue,
                },
                *modifiers,
            ),
            egui::Event::PointerMoved(pos) => {
                let button = if input.pointer.primary_down() {
                    MouseButton::Left
                } else if input.pointer.secondary_down() {
                    MouseButton::Right
                } else if input.pointer.middle_down() {
                    MouseButton::Middle
                } else {
                    MouseButton::None
                };
                if state.mouse_mode != MouseMode::AnyEvents && !(state.mouse_mode == MouseMode::ButtonEvents && button != MouseButton::None) {
                    continue;
                }
                (*pos, MouseEventType::Motion, button, input.modifiers)
            }
            egui::Event::MouseWheel { delta, modifiers, .. } if delta.y != 0.0 => {
                let Some(pos) = input.pointer.hover_pos() else {
                    continue;
                };
                (
                    pos,
                    MouseEventType::Press,
                    if delta.y > 0.0 { MouseButton::WheelUp } else { MouseButton::WheelDown },
                    *modifiers,
                )
            }
            _ => continue,
        };
        if modifiers.shift || (state.mouse_mode == MouseMode::X10 && kind != MouseEventType::Press) {
            continue;
        }
        let Some((column, row)) = render.screen_to_cell(position.x, position.y) else {
            continue;
        };
        let Some((pixel_x, pixel_y)) = render.screen_to_terminal_pixels(position.x, position.y) else {
            continue;
        };
        if pixel_x >= render.terminal_width || pixel_y >= render.terminal_height {
            continue;
        }
        let mut report = MouseEvent::new(state.clone());
        report.event_type = kind;
        report.position = Position::new(column, row);
        report.pixel_position = Some(Position::new(pixel_x as i32, (pixel_y / if render.scan_lines { 2.0 } else { 1.0 }) as i32));
        report.button = button;
        report.modifiers = KeyModifiers {
            shift: false,
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            meta: modifiers.mac_cmd,
        };
        if let Some(bytes) = report.generate_mouse_report() {
            if state.extended_mode == ExtMouseMode::None {
                output.extend(bytes.chars().map(|character| character as u8));
            } else {
                output.extend(bytes.as_bytes());
            }
        }
    }
    output
}

#[cfg(test)]
mod mouse_tests {
    use super::*;
    #[test]
    fn mouse_reports_respect_scale_profile_and_shift() {
        let context = egui::Context::default();
        let mut state = icy_engine::MouseState {
            mouse_tracking_enabled: true,
            mouse_mode: icy_engine::MouseMode::VT200,
            extended_mode: icy_engine::ExtMouseMode::SGR,
            ..Default::default()
        };
        let render = icy_engine_gui::terminal::RenderInfo {
            display_scale: 2.0,
            font_width: 8.0,
            font_height: 16.0,
            viewport_width: 640.0,
            viewport_height: 400.0,
            terminal_width: 320.0,
            terminal_height: 200.0,
            ..Default::default()
        };
        let events = vec![egui::Event::PointerButton {
            pos: egui::pos2(32.0, 64.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }];
        let _ = context.run(egui::RawInput { events, ..Default::default() }, |context| {
            assert_eq!(context.input(|input| encode_mouse_events(input, &state, &render)), b"\x1b[<0;3;3M");
            state.mouse_tracking_enabled = false;
            assert!(context.input(|input| encode_mouse_events(input, &state, &render)).is_empty());
        });
        state.mouse_tracking_enabled = true;
        let events = vec![egui::Event::PointerButton {
            pos: egui::pos2(32.0, 64.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::SHIFT,
        }];
        let _ = context.run(
            egui::RawInput {
                events,
                modifiers: egui::Modifiers::SHIFT,
                ..Default::default()
            },
            |context| {
                assert!(context.input(|input| encode_mouse_events(input, &state, &render)).is_empty());
            },
        );
    }
}
