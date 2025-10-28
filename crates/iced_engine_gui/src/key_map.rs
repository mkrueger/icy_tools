use iced::keyboard;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyWithModifiers {
    Named(keyboard::key::Named, bool, bool), // key, shift, ctrl
    Character(char, bool, bool),             // char, shift, ctrl
}

// Helper function to create entries
const fn named_key(key: keyboard::key::Named) -> KeyWithModifiers {
    KeyWithModifiers::Named(key, false, false)
}

const fn named_key_shift(key: keyboard::key::Named) -> KeyWithModifiers {
    KeyWithModifiers::Named(key, true, false)
}

const fn named_key_ctrl(key: keyboard::key::Named) -> KeyWithModifiers {
    KeyWithModifiers::Named(key, false, true)
}

const fn char_key(ch: char) -> KeyWithModifiers {
    KeyWithModifiers::Character(ch, false, false)
}

const fn char_key_shift(ch: char) -> KeyWithModifiers {
    KeyWithModifiers::Character(ch, true, false)
}

const fn char_key_ctrl(ch: char) -> KeyWithModifiers {
    KeyWithModifiers::Character(ch, false, true)
}

pub static ANSI_KEY_MAP: &[(KeyWithModifiers, &[u8])] = &[
    (named_key(keyboard::key::Named::Escape), &[0x1B]),
    (named_key(keyboard::key::Named::Home), b"\x1b[H"),
    (named_key(keyboard::key::Named::Insert), b"\x1b[@"),
    (named_key_shift(keyboard::key::Named::Insert), b" "),
    (named_key(keyboard::key::Named::Backspace), &[8]),
    (named_key(keyboard::key::Named::Enter), &[b'\r']),
    (named_key(keyboard::key::Named::Tab), &[9]),
    (named_key_shift(keyboard::key::Named::Tab), b"\x1b[Z"),
    (named_key(keyboard::key::Named::Delete), &[127]),
    (named_key_shift(keyboard::key::Named::Delete), &[127]),
    (named_key(keyboard::key::Named::End), b"\x1b[K"),
    (named_key_ctrl(keyboard::key::Named::End), &[11]),
    (named_key(keyboard::key::Named::PageUp), b"\x1b[V"),
    (named_key(keyboard::key::Named::PageDown), b"\x1b[U"),
    (named_key(keyboard::key::Named::F1), b"\x1b[OP"),
    (named_key(keyboard::key::Named::F2), b"\x1b[OQ"),
    (named_key(keyboard::key::Named::F3), b"\x1b[OR"),
    (named_key(keyboard::key::Named::F4), b"\x1b[OS"),
    (named_key(keyboard::key::Named::F5), b"\x1b[OT"),
    (named_key(keyboard::key::Named::F6), b"\x1b[17~"),
    (named_key(keyboard::key::Named::F7), b"\x1b[18~"),
    (named_key(keyboard::key::Named::F8), b"\x1b[19~"),
    (named_key(keyboard::key::Named::F9), b"\x1b[20~"),
    (named_key(keyboard::key::Named::F10), b"\x1b[21~"),
    (named_key(keyboard::key::Named::F11), b"\x1b[23~"),
    (named_key(keyboard::key::Named::F12), b"\x1b[24~"),
    (named_key(keyboard::key::Named::ArrowUp), b"\x1b[A"),
    (named_key(keyboard::key::Named::ArrowDown), b"\x1b[B"),
    (named_key(keyboard::key::Named::ArrowRight), b"\x1b[C"),
    (named_key(keyboard::key::Named::ArrowLeft), b"\x1b[D"),
    (named_key_shift(keyboard::key::Named::ArrowUp), b"\x1b[A"),
    (named_key_shift(keyboard::key::Named::ArrowDown), b"\x1b[B"),
    (named_key_shift(keyboard::key::Named::ArrowRight), b"\x1b[C"),
    (named_key_shift(keyboard::key::Named::ArrowLeft), b"\x1b[D"),
    (named_key_ctrl(keyboard::key::Named::ArrowRight), &[6]),
    (named_key_ctrl(keyboard::key::Named::ArrowLeft), &[1]),
    // Ctrl+Letter combinations
    (char_key_ctrl('a'), &[1]),
    (char_key_ctrl('b'), &[2]),
    (char_key_ctrl('c'), &[3]),
    (char_key_ctrl('d'), &[4]),
    (char_key_ctrl('e'), &[5]),
    (char_key_ctrl('f'), &[6]),
    (char_key_ctrl('g'), &[7]),
    (char_key_ctrl('h'), &[8]),
    (char_key_ctrl('i'), &[9]),
    (char_key_ctrl('j'), &[10]),
    (char_key_ctrl('k'), &[11]),
    (char_key_ctrl('l'), &[12]),
    (char_key_ctrl('m'), &[13]),
    (char_key_ctrl('n'), &[14]),
    (char_key_ctrl('o'), &[15]),
    (char_key_ctrl('p'), &[16]),
    (char_key_ctrl('q'), &[17]),
    (char_key_ctrl('r'), &[18]),
    (char_key_ctrl('s'), &[19]),
    (char_key_ctrl('t'), &[20]),
    (char_key_ctrl('u'), &[21]),
    (char_key_ctrl('v'), &[22]),
    (char_key_ctrl('w'), &[23]),
    (char_key_ctrl('x'), &[24]),
    (char_key_ctrl('y'), &[25]),
    (char_key_ctrl('z'), &[26]),
    // Ctrl+Number combinations
    (char_key_ctrl('2'), &[0]),
    (char_key_ctrl('3'), &[0x1B]),
    (char_key_ctrl('4'), &[0x1C]),
    (char_key_ctrl('5'), &[0x1D]),
    (char_key_ctrl('6'), &[0x1E]),
    (char_key_ctrl('7'), &[0x1F]),
    (char_key_ctrl('\\'), &[0x1C]),
    (char_key_ctrl('['), &[0x1B]),
    (char_key_ctrl(']'), &[0x1D]),
    (char_key_ctrl('-'), &[0x1F]),
];

pub static C64_KEY_MAP: &[(KeyWithModifiers, &[u8])] = &[
    (named_key(keyboard::key::Named::Escape), &[0x1B]),
    (named_key(keyboard::key::Named::Home), &[0x13]),
    (named_key_shift(keyboard::key::Named::Home), &[0x93]),
    (named_key(keyboard::key::Named::Enter), &[b'\r']),
    (named_key_shift(keyboard::key::Named::Enter), &[141]),
    (named_key(keyboard::key::Named::Insert), &[0x94]),
    (named_key(keyboard::key::Named::Backspace), &[0x14]),
    (named_key(keyboard::key::Named::Delete), &[0x14]),
    (named_key_shift(keyboard::key::Named::Delete), &[148]),
    (named_key(keyboard::key::Named::F1), &[0x85]),
    (named_key(keyboard::key::Named::F2), &[0x86]),
    (named_key(keyboard::key::Named::F3), &[0x87]),
    (named_key(keyboard::key::Named::F4), &[0x88]),
    (named_key(keyboard::key::Named::F5), &[0x89]),
    (named_key(keyboard::key::Named::F6), &[0x8A]),
    (named_key(keyboard::key::Named::F7), &[0x8B]),
    (named_key(keyboard::key::Named::F8), &[0x8C]),
    (named_key_shift(keyboard::key::Named::F1), &[137]),
    (named_key_shift(keyboard::key::Named::F3), &[138]),
    (named_key_shift(keyboard::key::Named::F5), &[139]),
    (named_key_shift(keyboard::key::Named::F7), &[140]),
    (named_key(keyboard::key::Named::ArrowUp), &[0x91]),
    (named_key(keyboard::key::Named::ArrowDown), &[0x11]),
    (named_key(keyboard::key::Named::ArrowRight), &[0x1D]),
    (named_key(keyboard::key::Named::ArrowLeft), &[0x9D]),
    (named_key_shift(keyboard::key::Named::ArrowUp), &[0x91]),
    (named_key_shift(keyboard::key::Named::ArrowDown), &[0x11]),
    (named_key_shift(keyboard::key::Named::ArrowRight), &[0x1D]),
    (named_key_shift(keyboard::key::Named::ArrowLeft), &[0x9D]),
    // Ctrl+Letter combinations
    (char_key_ctrl('a'), &[0x81]),
    (char_key_ctrl('b'), &[0x82]),
    (char_key_ctrl('c'), &[3]),
    (char_key_ctrl('d'), &[0x84]),
    (char_key_ctrl('e'), &[0x85]),
    (char_key_ctrl('f'), &[0x86]),
    (char_key_ctrl('g'), &[0x87]),
    (char_key_ctrl('h'), &[8]),
    (char_key_ctrl('i'), &[9]),
    (char_key_ctrl('j'), &[0x8A]),
    (char_key_ctrl('k'), &[0x8B]),
    (char_key_ctrl('l'), &[0x8C]),
    (char_key_ctrl('m'), &[13]),
    (char_key_ctrl('n'), &[14]),
    (char_key_ctrl('o'), &[0x8F]),
    (char_key_ctrl('p'), &[0x90]),
    (char_key_ctrl('q'), &[0x91]),
    (char_key_ctrl('r'), &[0x92]),
    (char_key_ctrl('s'), &[0x93]),
    (char_key_ctrl('t'), &[20]),
    (char_key_ctrl('u'), &[0x95]),
    (char_key_ctrl('v'), &[0x96]),
    (char_key_ctrl('w'), &[0x97]),
    (char_key_ctrl('x'), &[0x98]),
    (char_key_ctrl('y'), &[0x99]),
    (char_key_ctrl('z'), &[0x9A]),
    // Shift+Number combinations
    (char_key_shift('1'), &[0x21]),
    (char_key_shift('2'), &[0x22]),
    (char_key_shift('3'), &[0x23]),
    (char_key_shift('4'), &[0x24]),
    (char_key_shift('5'), &[0x25]),
    (char_key_shift('6'), &[0x26]),
    (char_key_shift('7'), &[0x27]),
    (char_key_shift('8'), &[0x28]),
    (char_key_shift('9'), &[0x29]),
    // Ctrl+Number combinations
    (char_key_ctrl('1'), &[144]),
    (char_key_ctrl('2'), &[5]),
    (char_key_ctrl('3'), &[28]),
    (char_key_ctrl('4'), &[159]),
    (char_key_ctrl('5'), &[156]),
    (char_key_ctrl('6'), &[30]),
    (char_key_ctrl('7'), &[31]),
    (char_key_ctrl('8'), &[158]),
    (char_key_ctrl('9'), &[18]),
    (char_key_ctrl('0'), &[146]),
];

pub static ATASCII_KEY_MAP: &[(KeyWithModifiers, &[u8])] = &[
    (named_key(keyboard::key::Named::Escape), &[0x1B]),
    (named_key(keyboard::key::Named::Enter), &[155]),
    (named_key(keyboard::key::Named::Backspace), &[0x1b, 0x7e]),
    (named_key(keyboard::key::Named::End), &[0x1b, 0x9b]),
    (named_key(keyboard::key::Named::ArrowUp), &[0x1b, 0x1c]),
    (named_key(keyboard::key::Named::ArrowDown), &[0x1b, 0x1d]),
    (named_key(keyboard::key::Named::ArrowRight), &[0x1b, 0x1f]),
    (named_key(keyboard::key::Named::ArrowLeft), &[0x1b, 0x1e]),
    (named_key_shift(keyboard::key::Named::ArrowUp), &[0x1b, 0x1c]),
    (named_key_shift(keyboard::key::Named::ArrowDown), &[0x1b, 0x1d]),
    (named_key_shift(keyboard::key::Named::ArrowRight), &[0x1b, 0x1f]),
    (named_key_shift(keyboard::key::Named::ArrowLeft), &[0x1b, 0x1e]),
    // Ctrl+Letter combinations
    (char_key_ctrl('a'), &[1]),
    (char_key_ctrl('b'), &[2]),
    (char_key_ctrl('c'), &[3]),
    (char_key_ctrl('d'), &[4]),
    (char_key_ctrl('e'), &[5]),
    (char_key_ctrl('f'), &[6]),
    (char_key_ctrl('g'), &[7]),
    (char_key_ctrl('h'), &[8]),
    (char_key_ctrl('i'), &[9]),
    (char_key_ctrl('j'), &[10]),
    (char_key_ctrl('k'), &[11]),
    (char_key_ctrl('l'), &[12]),
    (char_key_ctrl('m'), &[13]),
    (char_key_ctrl('n'), &[14]),
    (char_key_ctrl('o'), &[15]),
    (char_key_ctrl('p'), &[16]),
    (char_key_ctrl('q'), &[17]),
    (char_key_ctrl('r'), &[18]),
    (char_key_ctrl('s'), &[19]),
    (char_key_ctrl('t'), &[20]),
    (char_key_ctrl('u'), &[21]),
    (char_key_ctrl('v'), &[22]),
    (char_key_ctrl('w'), &[23]),
    (char_key_ctrl('x'), &[24]),
    (char_key_ctrl('y'), &[25]),
    (char_key_ctrl('z'), &[26]),
];

pub static VIDEOTERM_KEY_MAP: &[(KeyWithModifiers, &[u8])] = &[
    (named_key(keyboard::key::Named::Home), &[0x13]),
    (named_key(keyboard::key::Named::Enter), &[b'_']),
    (named_key(keyboard::key::Named::Insert), &[0x94]),
    (named_key(keyboard::key::Named::Backspace), &[0x7F]),
    (named_key(keyboard::key::Named::Delete), &[0x7F]),
    (named_key(keyboard::key::Named::Escape), &[0x1B]),
    (named_key(keyboard::key::Named::F1), &[b'*']),
    (named_key(keyboard::key::Named::F2), &[0b101_1111]),
    (named_key(keyboard::key::Named::ArrowUp), &[0x0B]),
    (named_key(keyboard::key::Named::ArrowDown), &[b'\n']),
    (named_key(keyboard::key::Named::ArrowRight), &[b'\t']),
    (named_key(keyboard::key::Named::ArrowLeft), &[0x08]),
    (named_key_shift(keyboard::key::Named::ArrowUp), &[0x0B]),
    (named_key_shift(keyboard::key::Named::ArrowDown), &[b'\n']),
    (named_key_shift(keyboard::key::Named::ArrowRight), &[b'\t']),
    (named_key_shift(keyboard::key::Named::ArrowLeft), &[0x08]),
];

pub fn lookup_key(key: &keyboard::Key, modifiers: keyboard::Modifiers, map: &[(KeyWithModifiers, &[u8])]) -> Option<Vec<u8>> {
    let key_with_mods = match key {
        keyboard::Key::Named(named) => KeyWithModifiers::Named(*named, modifiers.shift(), modifiers.control() || modifiers.command()),
        keyboard::Key::Character(s) => {
            let ch = s.chars().next()?;
            KeyWithModifiers::Character(ch.to_lowercase().next()?, modifiers.shift(), modifiers.control() || modifiers.command())
        }
        _ => return None,
    };

    for (mapped_key, bytes) in map {
        if *mapped_key == key_with_mods {
            return Some(bytes.to_vec());
        }
    }

    None
}
