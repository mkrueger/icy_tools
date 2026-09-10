use icy_engine::KeyModifiers;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamedKey {
    Escape,
    Home,
    Insert,
    Backspace,
    Enter,
    Tab,
    Delete,
    End,
    PageUp,
    PageDown,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    ArrowUp,
    ArrowDown,
    ArrowRight,
    ArrowLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Code {
    NumpadEnter,
    Backquote,
    NumpadMultiply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Named(NamedKey),
    Character(String),
    Unidentified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyWithModifiers {
    Named(NamedKey, bool, bool), // key, shift, ctrl
    Character(char, bool, bool), // char, shift, ctrl
    KeyCode(Code, bool, bool),
}

// Helper function to create entries
const fn named_key(key: NamedKey) -> KeyWithModifiers {
    KeyWithModifiers::Named(key, false, false)
}

const fn named_key_shift(key: NamedKey) -> KeyWithModifiers {
    KeyWithModifiers::Named(key, true, false)
}

const fn named_key_ctrl(key: NamedKey) -> KeyWithModifiers {
    KeyWithModifiers::Named(key, false, true)
}

const fn char_key_shift(ch: char) -> KeyWithModifiers {
    KeyWithModifiers::Character(ch, true, false)
}

const fn char_key_ctrl(ch: char) -> KeyWithModifiers {
    KeyWithModifiers::Character(ch, false, true)
}

const fn char_key(ch: char) -> KeyWithModifiers {
    KeyWithModifiers::Character(ch, false, false)
}

const fn key_code_key(code: Code) -> KeyWithModifiers {
    KeyWithModifiers::KeyCode(code, false, false)
}

pub static ANSI_KEY_MAP: &[(KeyWithModifiers, &[u8])] = &[
    (named_key(NamedKey::Escape), &[0x1B]),
    (named_key(NamedKey::Home), b"\x1b[H"),
    (named_key(NamedKey::Insert), b"\x1b[@"),
    (named_key_shift(NamedKey::Insert), b" "),
    (named_key(NamedKey::Backspace), &[8]),
    (named_key(NamedKey::Enter), &[b'\r']),
    (named_key(NamedKey::Tab), &[9]),
    (named_key_shift(NamedKey::Tab), b"\x1b[Z"),
    (named_key(NamedKey::Delete), &[127]),
    (named_key_shift(NamedKey::Delete), &[127]),
    (named_key(NamedKey::End), b"\x1b[K"),
    (named_key_ctrl(NamedKey::End), &[11]),
    (named_key(NamedKey::PageUp), b"\x1b[V"),
    (named_key(NamedKey::PageDown), b"\x1b[U"),
    (named_key(NamedKey::F1), b"\x1b[OP"),
    (named_key(NamedKey::F2), b"\x1b[OQ"),
    (named_key(NamedKey::F3), b"\x1b[OR"),
    (named_key(NamedKey::F4), b"\x1b[OS"),
    (named_key(NamedKey::F5), b"\x1b[OT"),
    (named_key(NamedKey::F6), b"\x1b[17~"),
    (named_key(NamedKey::F7), b"\x1b[18~"),
    (named_key(NamedKey::F8), b"\x1b[19~"),
    (named_key(NamedKey::F9), b"\x1b[20~"),
    (named_key(NamedKey::F10), b"\x1b[21~"),
    (named_key(NamedKey::F11), b"\x1b[23~"),
    (named_key(NamedKey::F12), b"\x1b[24~"),
    (named_key(NamedKey::ArrowUp), b"\x1b[A"),
    (named_key(NamedKey::ArrowDown), b"\x1b[B"),
    (named_key(NamedKey::ArrowRight), b"\x1b[C"),
    (named_key(NamedKey::ArrowLeft), b"\x1b[D"),
    (named_key_shift(NamedKey::ArrowUp), b"\x1b[A"),
    (named_key_shift(NamedKey::ArrowDown), b"\x1b[B"),
    (named_key_shift(NamedKey::ArrowRight), b"\x1b[C"),
    (named_key_shift(NamedKey::ArrowLeft), b"\x1b[D"),
    (named_key_ctrl(NamedKey::ArrowRight), &[6]),
    (named_key_ctrl(NamedKey::ArrowLeft), &[1]),
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
    (named_key(NamedKey::Escape), &[0x1B]),
    (named_key(NamedKey::Home), &[0x13]),
    (named_key_shift(NamedKey::Home), &[0x93]),
    (named_key(NamedKey::Enter), &[b'\r']),
    (named_key_shift(NamedKey::Enter), &[141]),
    (named_key(NamedKey::Insert), &[0x94]),
    (named_key(NamedKey::Backspace), &[0x14]),
    (named_key(NamedKey::Delete), &[0x14]),
    (named_key_shift(NamedKey::Delete), &[148]),
    (named_key(NamedKey::F1), &[0x85]),
    (named_key(NamedKey::F2), &[0x86]),
    (named_key(NamedKey::F3), &[0x87]),
    (named_key(NamedKey::F4), &[0x88]),
    (named_key(NamedKey::F5), &[0x89]),
    (named_key(NamedKey::F6), &[0x8A]),
    (named_key(NamedKey::F7), &[0x8B]),
    (named_key(NamedKey::F8), &[0x8C]),
    (named_key_shift(NamedKey::F1), &[137]),
    (named_key_shift(NamedKey::F3), &[138]),
    (named_key_shift(NamedKey::F5), &[139]),
    (named_key_shift(NamedKey::F7), &[140]),
    (named_key(NamedKey::ArrowUp), &[0x91]),
    (named_key(NamedKey::ArrowDown), &[0x11]),
    (named_key(NamedKey::ArrowRight), &[0x1D]),
    (named_key(NamedKey::ArrowLeft), &[0x9D]),
    (named_key_shift(NamedKey::ArrowUp), &[0x91]),
    (named_key_shift(NamedKey::ArrowDown), &[0x11]),
    (named_key_shift(NamedKey::ArrowRight), &[0x1D]),
    (named_key_shift(NamedKey::ArrowLeft), &[0x9D]),
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
    (named_key(NamedKey::Escape), &[0x1B]),
    (named_key(NamedKey::Enter), &[155]),
    (named_key(NamedKey::Backspace), &[0x1b, 0x7e]),
    (named_key(NamedKey::End), &[0x1b, 0x9b]),
    (named_key(NamedKey::ArrowUp), &[0x1b, 0x1c]),
    (named_key(NamedKey::ArrowDown), &[0x1b, 0x1d]),
    (named_key(NamedKey::ArrowRight), &[0x1b, 0x1f]),
    (named_key(NamedKey::ArrowLeft), &[0x1b, 0x1e]),
    (named_key_shift(NamedKey::ArrowUp), &[0x1b, 0x1c]),
    (named_key_shift(NamedKey::ArrowDown), &[0x1b, 0x1d]),
    (named_key_shift(NamedKey::ArrowRight), &[0x1b, 0x1f]),
    (named_key_shift(NamedKey::ArrowLeft), &[0x1b, 0x1e]),
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
    (char_key('#'), b"_"),
    (char_key('_'), b"`"),
    (char_key('`'), b"#"),
    (char_key_ctrl('l'), &[0x0C]),
    (char_key_ctrl('t'), &[0x18]),
    (named_key(NamedKey::Home), &[0x1E]),
    (named_key(NamedKey::Enter), &[b'_']),
    (named_key(NamedKey::Insert), &[0x94]),
    (named_key(NamedKey::Backspace), &[0x7F]),
    (named_key(NamedKey::Delete), &[0x7F]),
    (named_key(NamedKey::Escape), &[0x1B]),
    (named_key(NamedKey::F2), &[b'_']),         // F2 duplicates Enter (Send) – confirm
    (named_key(NamedKey::F7), &[0x1B]),         // F7 acts as ESC (Commstar)
    (key_code_key(Code::NumpadEnter), &[b'_']), // Numpad Enter same as Enter
    (key_code_key(Code::Backquote), &[b'*']),
    (key_code_key(Code::NumpadMultiply), &[b'*']),
    (named_key(NamedKey::ArrowUp), &[0x0B]),
    (named_key(NamedKey::ArrowDown), &[b'\n']),
    (named_key(NamedKey::ArrowRight), &[b'\t']),
    (named_key(NamedKey::ArrowLeft), &[0x08]),
    (named_key_shift(NamedKey::ArrowUp), &[0x0B]),
    (named_key_shift(NamedKey::ArrowDown), &[b'\n']),
    (named_key_shift(NamedKey::ArrowRight), &[b'\t']),
    (named_key_shift(NamedKey::ArrowLeft), &[0x08]),
    (named_key_shift(NamedKey::Enter), &[13]), // Shift+Enter sends CR
];

pub static MODE7_KEY_MAP: &[(KeyWithModifiers, &[u8])] = &[
    // Basic controls
    (named_key(NamedKey::Escape), &[0x1B]),   // ESC
    (named_key(NamedKey::Home), &[0x1F]),     // HOME -> 0x1F (TAB to x,y / Unit Separator in spec usage)
    (named_key(NamedKey::Backspace), &[127]), // Destructive backspace
    (named_key(NamedKey::Delete), &[127]),    // Treat Delete same as destructive backspace
    (named_key(NamedKey::Tab), &[9]),         // TAB
    (named_key(NamedKey::Enter), &[13]),      // Enter -> CR
    (char_key_ctrl('a'), &[0x01]),
    (char_key_ctrl('b'), &[0x02]),
    (char_key_ctrl('c'), &[0x03]),
    (char_key_ctrl('d'), &[0x04]),
    (char_key_ctrl('e'), &[0x05]),
    (char_key_ctrl('f'), &[0x06]),
    (char_key_ctrl('g'), &[0x07]),
    (char_key_ctrl('h'), &[0x08]),
    (char_key_ctrl('i'), &[0x09]),
    (char_key_ctrl('j'), &[0x0A]),
    (char_key_ctrl('k'), &[0x0B]),
    (char_key_ctrl('l'), &[0x0C]),
    (char_key_ctrl('m'), &[0x0D]),
    (char_key_ctrl('n'), &[0x0E]),
    (char_key_ctrl('o'), &[0x0F]),
    (char_key_ctrl('p'), &[0x10]),
    (char_key_ctrl('q'), &[0x11]),
    (char_key_ctrl('r'), &[0x12]),
    (char_key_ctrl('s'), &[0x13]),
    (char_key_ctrl('t'), &[0x14]),
    (char_key_ctrl('u'), &[0x15]),
    (char_key_ctrl('v'), &[0x16]),
    (char_key_ctrl('w'), &[0x17]),
    (char_key_ctrl('x'), &[0x18]),
    (char_key_ctrl('y'), &[0x19]),
    (char_key_ctrl('z'), &[0x1A]),
    // Arrow keys (unmodified)
    (named_key(NamedKey::ArrowLeft), &[140]),
    (named_key(NamedKey::ArrowRight), &[141]),
    (named_key(NamedKey::ArrowDown), &[142]),
    (named_key(NamedKey::ArrowUp), &[143]),
    // Shift + Function keys (F0 = Shift+F10)
    (named_key_shift(NamedKey::F10), &[144]), // F0 (Shift F10)
    (named_key_shift(NamedKey::F1), &[145]),
    (named_key_shift(NamedKey::F2), &[146]),
    (named_key_shift(NamedKey::F3), &[147]),
    (named_key_shift(NamedKey::F4), &[148]),
    (named_key_shift(NamedKey::F5), &[149]),
    (named_key_shift(NamedKey::F6), &[150]),
    (named_key_shift(NamedKey::F7), &[151]),
    (named_key_shift(NamedKey::F8), &[152]),
    (named_key_shift(NamedKey::F9), &[153]),
    // Shift + navigation / edit
    (named_key_shift(NamedKey::End), &[155]), // Copy
    (named_key_shift(NamedKey::ArrowLeft), &[156]),
    (named_key_shift(NamedKey::ArrowRight), &[157]),
    (named_key_shift(NamedKey::ArrowDown), &[158]),
    (named_key_shift(NamedKey::ArrowUp), &[159]),
    // Ctrl + Function keys (Ctrl F10 first)
    (named_key_ctrl(NamedKey::F10), &[160]),
    (named_key_ctrl(NamedKey::F1), &[161]),
    (named_key_ctrl(NamedKey::F2), &[162]),
    (named_key_ctrl(NamedKey::F3), &[163]),
    (named_key_ctrl(NamedKey::F4), &[164]),
    (named_key_ctrl(NamedKey::F5), &[165]),
    (named_key_ctrl(NamedKey::F6), &[166]),
    (named_key_ctrl(NamedKey::F7), &[167]),
    (named_key_ctrl(NamedKey::F8), &[168]),
    (named_key_ctrl(NamedKey::F9), &[169]),
    // Ctrl + navigation / edit
    (named_key_ctrl(NamedKey::End), &[171]),
    (named_key_ctrl(NamedKey::ArrowLeft), &[172]),
    (named_key_ctrl(NamedKey::ArrowRight), &[173]),
    (named_key_ctrl(NamedKey::ArrowDown), &[174]),
    (named_key_ctrl(NamedKey::ArrowUp), &[175]),
    // Special: F7 (unmodified) maps to ESC per Commstar (another ESC)
    (named_key(NamedKey::F7), &[27]),
];

pub static ATARI_ST_KEY_MAP: &[(KeyWithModifiers, &[u8])] = &[
    // Function keys (VT52 style)
    (named_key(NamedKey::F1), b"\x1bP"), // ESC P
    (named_key(NamedKey::F2), b"\x1bQ"), // ESC Q
    (named_key(NamedKey::F3), b"\x1bR"), // ESC R
    // Arrow keys (VT52 cursor movement)
    (named_key(NamedKey::ArrowUp), b"\x1bA"),    // ESC A
    (named_key(NamedKey::ArrowDown), b"\x1bB"),  // ESC B
    (named_key(NamedKey::ArrowRight), b"\x1bC"), // ESC C
    (named_key(NamedKey::ArrowLeft), b"\x1bD"),  // ESC D
    // Shift + Arrows (same codes as unshifted for VT52)
    (named_key_shift(NamedKey::ArrowUp), b"\x1bA"),
    (named_key_shift(NamedKey::ArrowDown), b"\x1bB"),
    (named_key_shift(NamedKey::ArrowRight), b"\x1bC"),
    (named_key_shift(NamedKey::ArrowLeft), b"\x1bD"),
    // Delete key - sends ASCII 127 (DEL)
    // Note: The C code checks DECBKM flag, but defaults to 0x7f
    (named_key(NamedKey::Delete), &[0x7F]),
    // Backspace - typically sends 0x08 for VT52
    // Note: C code shows DECBKM flag affects this, defaulting to 0x7F without flag
    (named_key(NamedKey::Backspace), &[0x08]),
    // Standard keys
    (named_key(NamedKey::Escape), &[0x1B]), // ESC
    (named_key(NamedKey::Enter), &[0x0D]),  // CR
    (named_key(NamedKey::Tab), &[0x09]),    // TAB
    // Home/End - VT52 style
    (named_key(NamedKey::Home), b"\x1bH"), // ESC H (cursor home)
    (named_key(NamedKey::End), b"\x1bE"),  // ESC E (clear screen)
    // Insert - VT52 doesn't have standard insert, using ESC @
    (named_key(NamedKey::Insert), b"\x1b@"),
    // Page Up/Down - not standard in VT52, but common extensions
    (named_key(NamedKey::PageUp), b"\x1bI"),   // ESC I (reverse line feed)
    (named_key(NamedKey::PageDown), b"\x1bJ"), // ESC J (clear to end of screen)
    // Additional function keys (F4-F10) - common Atari ST extensions
    (named_key(NamedKey::F4), b"\x1bS"),
    (named_key(NamedKey::F5), b"\x1bT"),
    (named_key(NamedKey::F6), b"\x1bU"),
    (named_key(NamedKey::F7), b"\x1bV"),
    (named_key(NamedKey::F8), b"\x1bW"),
    (named_key(NamedKey::F9), b"\x1bX"),
    (named_key(NamedKey::F10), b"\x1bY"),
    // Ctrl+Letter combinations (standard ASCII control codes)
    (char_key_ctrl('a'), &[0x01]),
    (char_key_ctrl('b'), &[0x02]),
    (char_key_ctrl('c'), &[0x03]),
    (char_key_ctrl('d'), &[0x04]),
    (char_key_ctrl('e'), &[0x05]),
    (char_key_ctrl('f'), &[0x06]),
    (char_key_ctrl('g'), &[0x07]),
    (char_key_ctrl('h'), &[0x08]),
    (char_key_ctrl('i'), &[0x09]),
    (char_key_ctrl('j'), &[0x0A]),
    (char_key_ctrl('k'), &[0x0B]),
    (char_key_ctrl('l'), &[0x0C]),
    (char_key_ctrl('m'), &[0x0D]),
    (char_key_ctrl('n'), &[0x0E]),
    (char_key_ctrl('o'), &[0x0F]),
    (char_key_ctrl('p'), &[0x10]),
    (char_key_ctrl('q'), &[0x11]),
    (char_key_ctrl('r'), &[0x12]),
    (char_key_ctrl('s'), &[0x13]),
    (char_key_ctrl('t'), &[0x14]),
    (char_key_ctrl('u'), &[0x15]),
    (char_key_ctrl('v'), &[0x16]),
    (char_key_ctrl('w'), &[0x17]),
    (char_key_ctrl('x'), &[0x18]),
    (char_key_ctrl('y'), &[0x19]),
    (char_key_ctrl('z'), &[0x1A]),
    // Ctrl+[ = ESC, Ctrl+\ = FS, Ctrl+] = GS, Ctrl+^ = RS, Ctrl+_ = US
    (char_key_ctrl('['), &[0x1B]),
    (char_key_ctrl('\\'), &[0x1C]),
    (char_key_ctrl(']'), &[0x1D]),
    (char_key_ctrl('^'), &[0x1E]),
    (char_key_ctrl('_'), &[0x1F]),
    // Ctrl+Number combinations
    (char_key_ctrl('2'), &[0x00]), // Ctrl+2 = NUL
    (char_key_ctrl('3'), &[0x1B]), // Ctrl+3 = ESC
    (char_key_ctrl('4'), &[0x1C]), // Ctrl+4 = FS
    (char_key_ctrl('5'), &[0x1D]), // Ctrl+5 = GS
    (char_key_ctrl('6'), &[0x1E]), // Ctrl+6 = RS
    (char_key_ctrl('7'), &[0x1F]), // Ctrl+7 = US
    (char_key_ctrl('8'), &[0x7F]), // Ctrl+8 = DEL
];

pub fn ansi_modified_function_key(key: NamedKey, modifiers: KeyModifiers) -> Option<Vec<u8>> {
    let modifier = 1 + u8::from(modifiers.shift) + 2 * u8::from(modifiers.alt) + 4 * u8::from(modifiers.ctrl);
    if modifier == 1 {
        return None;
    }

    let sequence = match key {
        NamedKey::F1 => format!("\x1b[1;{modifier}P"),
        NamedKey::F2 => format!("\x1b[1;{modifier}Q"),
        NamedKey::F3 => format!("\x1b[1;{modifier}R"),
        NamedKey::F4 => format!("\x1b[1;{modifier}S"),
        NamedKey::F5 => format!("\x1b[15;{modifier}~"),
        NamedKey::F6 => format!("\x1b[17;{modifier}~"),
        NamedKey::F7 => format!("\x1b[18;{modifier}~"),
        NamedKey::F8 => format!("\x1b[19;{modifier}~"),
        NamedKey::F9 => format!("\x1b[20;{modifier}~"),
        NamedKey::F10 => format!("\x1b[21;{modifier}~"),
        NamedKey::F11 => format!("\x1b[23;{modifier}~"),
        NamedKey::F12 => format!("\x1b[24;{modifier}~"),
        _ => return None,
    };
    Some(sequence.into_bytes())
}

pub fn lookup_key(key: &Key, physical: &Option<Code>, modifiers: KeyModifiers, map: &[(KeyWithModifiers, &[u8])]) -> Option<Vec<u8>> {
    let shift = modifiers.shift;
    let ctrl = modifiers.ctrl;

    if std::ptr::eq(map, ANSI_KEY_MAP) {
        if let Key::Named(named) = key {
            if let Some(sequence) = ansi_modified_function_key(*named, modifiers) {
                return Some(sequence);
            }
        }
    }

    // Always also check physical key code
    if let Some(code) = physical {
        let physical_key_with_mods = KeyWithModifiers::KeyCode(*code, shift, ctrl);
        for (mapped_key, bytes) in map {
            if *mapped_key == physical_key_with_mods {
                return Some(bytes.to_vec());
            }
        }
    }

    // Try logical key interpretations
    match key {
        Key::Named(named) => {
            let key_with_mods = KeyWithModifiers::Named(*named, shift, ctrl);
            for (mapped_key, bytes) in map {
                if *mapped_key == key_with_mods {
                    return Some(bytes.to_vec());
                }
            }
        }
        Key::Character(s) => {
            if let Some(ch) = s.chars().next() {
                // Try exact character
                let key_with_mods = KeyWithModifiers::Character(ch, shift, ctrl);
                for (mapped_key, bytes) in map {
                    if *mapped_key == key_with_mods {
                        return Some(bytes.to_vec());
                    }
                }

                // Try lowercase version
                if let Some(lower_ch) = ch.to_lowercase().next() {
                    if lower_ch != ch {
                        // Only try if actually different
                        let key_with_mods = KeyWithModifiers::Character(lower_ch, shift, ctrl);
                        for (mapped_key, bytes) in map {
                            if *mapped_key == key_with_mods {
                                return Some(bytes.to_vec());
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    None
}
