mod icy_clipboard_generator;
pub use icy_clipboard_generator::*;

mod rtf_generator;
pub use rtf_generator::*;

mod text_genartor;
pub use text_genartor::*;

use std::borrow::Cow;

use crate::{BufferType, TerminalEmulation};

/// Plain-text fallback for typographic symbols that pasted text commonly contains.
fn paste_fallback(ch: char) -> Option<&'static str> {
    Some(match ch {
        '\u{2018}' | '\u{2019}' => "'",
        '\u{201c}' | '\u{201d}' => "\"",
        '\u{2190}' => "<-",
        '\u{2192}' => "->",
        '\u{2013}' => "-",
        '\u{2014}' => "--",
        '\u{00a9}' => "(c)",
        '\u{00ae}' => "(r)",
        '\u{2122}' => "TM",
        '\u{00bc}' => "1/4",
        '\u{00bd}' => "1/2",
        '\u{00be}' => "3/4",
        '\u{2026}' => "...",
        '\u{02c6}' => "^",
        _ => return None,
    })
}

/// Whether `byte` is interpreted as a control code instead of a glyph in `buffer_type`.
fn is_control_byte(buffer_type: BufferType, byte: u32) -> bool {
    match buffer_type {
        BufferType::Atascii => matches!(byte, 0x1B..=0x1F | 0x7D..=0x7F | 0x9B..=0x9F | 0xFD..=0xFF),
        BufferType::Petscii => byte < 0x20 || (0x80..=0x9F).contains(&byte),
        _ => byte < 0x20 || byte == 0x7F,
    }
}

/// Whether `buffer_type` has a printable glyph for `ch` that converts back to `ch`.
fn has_native_glyph(buffer_type: BufferType, ch: char) -> bool {
    match buffer_type {
        BufferType::Unicode => true,
        // The PETSCII tables only map bytes, so non-ASCII matches are accidental.
        BufferType::Petscii if !ch.is_ascii() => false,
        _ => buffer_type
            .try_convert_from_unicode(ch)
            .is_some_and(|code| buffer_type.convert_to_unicode(code) == ch && !is_control_byte(buffer_type, code as u32)),
    }
}

fn replace_symbols(text: &str, keep: impl Fn(char) -> bool) -> Cow<'_, str> {
    let Some(first) = text.char_indices().find(|&(_, ch)| paste_fallback(ch).is_some() && !keep(ch)).map(|(i, _)| i) else {
        return Cow::Borrowed(text);
    };
    let mut result = String::with_capacity(text.len() + 8);
    result.push_str(&text[..first]);
    for ch in text[first..].chars() {
        match paste_fallback(ch).filter(|_| !keep(ch)) {
            Some(replacement) => result.push_str(replacement),
            None => result.push(ch),
        }
    }
    Cow::Owned(result)
}

/// Replaces typographic symbols that `buffer_type` cannot display with plain-text
/// equivalents before pasted text is encoded. Symbols the encoding has a printable
/// glyph for (such as `½` in CP437 or `←` in Viewdata) are kept, as are all unlisted
/// characters and every character of Unicode text.
pub fn normalize_paste_text(text: &str, buffer_type: BufferType) -> Cow<'_, str> {
    replace_symbols(text, |ch| has_native_glyph(buffer_type, ch))
}

/// [`normalize_paste_text`] for text sent to a terminal host. SkyPix screens use
/// Amiga fonts whose glyphs differ from the CP437 bytes their buffer reports, so
/// every listed symbol is replaced there.
pub fn normalize_terminal_paste_text(text: &str, buffer_type: BufferType, emulation: TerminalEmulation) -> Cow<'_, str> {
    if emulation == TerminalEmulation::Skypix {
        replace_symbols(text, |_| false)
    } else {
        normalize_paste_text(text, buffer_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMART_TEXT: &str = "\u{2018}Single smart quotes\u{2019} or \u{201c}Double smart quotes\u{201d} \u{2190} \u{2192} \u{2013} \u{2014} \u{00a9} \u{00ae} \u{2122} \u{00bc} \u{00bd} \u{00be} \u{2026} \u{02c6}";
    const PLAIN_TEXT: &str = "'Single smart quotes' or \"Double smart quotes\" <- -> - -- (c) (r) TM 1/4 1/2 3/4 ... ^";

    fn normalized(buffer_type: BufferType) -> String {
        normalize_paste_text(SMART_TEXT, buffer_type).into_owned()
    }

    #[test]
    fn cp437_keeps_its_fraction_glyphs() {
        assert_eq!(
            normalized(BufferType::CP437),
            "'Single smart quotes' or \"Double smart quotes\" <- -> - -- (c) (r) TM \u{00bc} \u{00bd} 3/4 ... ^"
        );
    }

    #[test]
    fn viewdata_keeps_arrows_and_fractions() {
        assert_eq!(
            normalized(BufferType::Viewdata),
            "'Single smart quotes' or \"Double smart quotes\" \u{2190} \u{2192} - -- (c) (r) TM \u{00bc} \u{00bd} \u{00be} ... ^"
        );
    }

    #[test]
    fn atari_st_keeps_its_symbol_glyphs() {
        assert_eq!(
            normalized(BufferType::AtariSt),
            "'Single smart quotes' or \"Double smart quotes\" <- -> - -- \u{00a9} \u{00ae} \u{2122} \u{00bc} \u{00bd} 3/4 ... ^"
        );
    }

    #[test]
    fn petscii_and_atascii_use_plain_text() {
        // ATASCII's arrows are cursor controls; PETSCII has none of these glyphs.
        assert_eq!(normalized(BufferType::Petscii), PLAIN_TEXT);
        assert_eq!(normalized(BufferType::Atascii), PLAIN_TEXT);
    }

    #[test]
    fn skypix_terminals_use_plain_text() {
        assert_eq!(
            normalize_terminal_paste_text(SMART_TEXT, BufferType::CP437, TerminalEmulation::Skypix),
            PLAIN_TEXT
        );
        assert_eq!(
            normalize_terminal_paste_text(SMART_TEXT, BufferType::CP437, TerminalEmulation::Ansi),
            normalized(BufferType::CP437)
        );
    }

    #[test]
    fn unicode_and_unlisted_text_are_borrowed_unchanged() {
        assert!(matches!(normalize_paste_text(SMART_TEXT, BufferType::Unicode), Cow::Borrowed(text) if text == SMART_TEXT));
        for text in ["", "ASCII\r\n\ttext", "\u{00e9}\u{2500}\u{2588}\u{1f600}", PLAIN_TEXT] {
            assert!(matches!(normalize_paste_text(text, BufferType::CP437), Cow::Borrowed(t) if t == text));
        }
    }
}
