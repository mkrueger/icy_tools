pub const DISAMBIGUATE: u8 = 0b1;
pub const REPORT_EVENT_TYPES: u8 = 0b10;
pub const REPORT_ALTERNATE_KEYS: u8 = 0b100;
pub const REPORT_ALL_KEYS: u8 = 0b1000;
pub const REPORT_ASSOCIATED_TEXT: u8 = 0b1_0000;
pub const SUPPORTED_FLAGS: u8 = DISAMBIGUATE | REPORT_EVENT_TYPES | REPORT_ALL_KEYS | REPORT_ASSOCIATED_TEXT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyId {
    Unicode(u32),
    Tilde(u32),
    Final(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    Press,
    Repeat,
    Release,
}

pub fn encode(flags: u8, id: KeyId, bits: u8, text: Option<&str>, kind: KeyEventKind, character: bool, modifier: bool) -> Option<Vec<u8>> {
    if flags == 0 {
        return None;
    }
    let report_events = flags & REPORT_EVENT_TYPES != 0;
    if kind != KeyEventKind::Press && !report_events {
        return None;
    }
    let all_keys = flags & REPORT_ALL_KEYS != 0;
    if modifier && !all_keys {
        return None;
    }
    if !all_keys && matches!(id, KeyId::Unicode(_)) && character && bits & 0b1110 == 0 {
        return None;
    }
    let mut sequence = String::from("\x1b[");
    let number = match id {
        KeyId::Unicode(number) | KeyId::Tilde(number) => number,
        KeyId::Final(_) => 1,
    };
    let event = match kind {
        KeyEventKind::Press => 1,
        KeyEventKind::Repeat => 2,
        KeyEventKind::Release => 3,
    };
    let associated = if flags & REPORT_ASSOCIATED_TEXT != 0 && kind != KeyEventKind::Release {
        text.filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
    } else {
        None
    };
    sequence.push_str(&number.to_string());
    if bits != 0 || (report_events && event != 1) || associated.is_some() {
        sequence.push(';');
        sequence.push_str(&(bits + 1).to_string());
        if report_events && event != 1 {
            sequence.push(':');
            sequence.push_str(&event.to_string());
        }
        if let Some(associated) = associated {
            sequence.push(';');
            sequence.push_str(&associated.chars().map(|character| (character as u32).to_string()).collect::<Vec<_>>().join(":"));
        }
    }
    sequence.push(match id {
        KeyId::Unicode(_) => 'u',
        KeyId::Tilde(_) => '~',
        KeyId::Final(final_byte) => final_byte as char,
    });
    Some(sequence.into_bytes())
}
