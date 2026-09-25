//! Header text from QWK packets is stored as raw bytes and only decoded when shown.

use std::borrow::Cow;

use icy_engine::BufferType;

/// Text for a header field: ASCII and valid UTF-8 are borrowed as is, anything
/// else is read as CP437.
pub fn decode(bytes: &[u8]) -> Cow<'_, str> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Cow::Borrowed(text),
        Err(_) => Cow::Owned(bytes.iter().map(|&byte| cp437_char(byte)).collect()),
    }
}

fn cp437_char(byte: u8) -> char {
    match byte {
        // Control codes have glyphs in CP437 fonts but not in UI fonts.
        0..=31 | 127 => ' ',
        byte => BufferType::CP437.convert_to_unicode(char::from(byte)),
    }
}

/// Case-insensitive ordering of two header fields without decoding them.
pub fn cmp_ignore_case(left: &[u8], right: &[u8]) -> std::cmp::Ordering {
    left.iter().map(u8::to_ascii_lowercase).cmp(right.iter().map(u8::to_ascii_lowercase))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_ascii_utf8_and_cp437() {
        assert!(matches!(decode(b"Alice"), Cow::Borrowed("Alice")));
        assert_eq!(decode("Andr\u{e9}".as_bytes()), "Andr\u{e9}");
        assert_eq!(decode(b"Andr\x82 \xb0\xdb"), "Andr\u{e9} \u{2591}\u{2588}");
        assert_eq!(decode(b"a\x01b"), "a b");
        assert_eq!(cmp_ignore_case(b"alice", b"Bob"), std::cmp::Ordering::Less);
    }
}
