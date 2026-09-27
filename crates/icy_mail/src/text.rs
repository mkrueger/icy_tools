//! QWK header text, decoded for searching and optionally styled by ANSI attributes.

use std::{fmt, ops::Deref};

use bstr::BString;
use icy_engine::{AttributeColor, BufferType, EditableScreen, Position, Screen, Size, TextPane, TextScreen, XTERM_256_PALETTE};

const HEADER_WIDTH: i32 = 160;
const HEADER_HEIGHT: i32 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyledSpan {
    pub text: String,
    pub foreground: Option<[u8; 3]>,
    pub background: Option<[u8; 3]>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

#[derive(Clone, Debug, Default)]
pub struct HeaderText {
    plain: String,
    styled: Option<Box<[StyledSpan]>>,
}

impl HeaderText {
    #[must_use]
    pub fn new(bytes: &[u8]) -> Self {
        if bytes.iter().any(u8::is_ascii_control) {
            return parse_ansi(bytes);
        }

        Self {
            plain: decode_unstyled(bytes),
            styled: None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.plain
    }

    #[must_use]
    pub fn styled(&self) -> Option<&[StyledSpan]> {
        self.styled.as_deref()
    }
}

impl Deref for HeaderText {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for HeaderText {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for HeaderText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<str> for HeaderText {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for HeaderText {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl From<&[u8]> for HeaderText {
    fn from(value: &[u8]) -> Self {
        Self::new(value)
    }
}

impl<const N: usize> From<&[u8; N]> for HeaderText {
    fn from(value: &[u8; N]) -> Self {
        Self::new(value)
    }
}

impl From<BString> for HeaderText {
    fn from(value: BString) -> Self {
        Self::new(&value)
    }
}

impl From<String> for HeaderText {
    fn from(value: String) -> Self {
        Self::new(value.as_bytes())
    }
}

impl From<&str> for HeaderText {
    fn from(value: &str) -> Self {
        Self::new(value.as_bytes())
    }
}

pub fn cmp_ignore_case(left: &HeaderText, right: &HeaderText) -> std::cmp::Ordering {
    if left.is_ascii() && right.is_ascii() {
        left.bytes()
            .map(|byte| byte.to_ascii_lowercase())
            .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase()))
    } else {
        left.chars().flat_map(char::to_lowercase).cmp(right.chars().flat_map(char::to_lowercase))
    }
}

/// Byte ranges of the non-overlapping case-insensitive matches of `needle` in `text`,
/// used to highlight search results.
#[must_use]
pub fn find_ignore_case(text: &str, needle: &str) -> Vec<std::ops::Range<usize>> {
    let needle: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    let mut matches = Vec::new();
    if needle.is_empty() {
        return matches;
    }
    let mut next = 0;
    for (start, _) in text.char_indices() {
        if start < next {
            continue;
        }
        let mut wanted = needle.iter();
        let mut end = start;
        let mut pending = wanted.len();
        for ch in text[start..].chars() {
            if !ch.to_lowercase().all(|lower| wanted.next() == Some(&lower)) {
                break;
            }
            end += ch.len_utf8();
            pending = wanted.len();
            if pending == 0 {
                break;
            }
        }
        if pending == 0 {
            matches.push(start..end);
            next = end;
        }
    }
    matches
}

fn decode_unstyled(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => bytes.iter().map(|&byte| cp437_char(byte)).collect(),
    }
}

fn cp437_char(byte: u8) -> char {
    BufferType::CP437.convert_to_unicode(char::from(byte))
}

fn parse_ansi(bytes: &[u8]) -> HeaderText {
    let mut screen = TextScreen::new(Size::new(HEADER_WIDTH, HEADER_HEIGHT));
    screen.terminal_state_mut().is_terminal_buffer = false;
    if let Err(error) = icy_engine::load_with_parser(&mut screen, &mut icy_parser_core::AnsiParser::new(), bytes, true, -1) {
        log::warn!("unable to parse ANSI in QWK header: {error}");
        return HeaderText {
            plain: decode_unstyled(bytes),
            styled: None,
        };
    }

    let mut rows = Vec::new();
    for y in 0..HEADER_HEIGHT {
        let mut end = 0;
        for x in 0..HEADER_WIDTH {
            let cell = screen.char_at(Position::new(x, y));
            if cell.ch != ' ' && cell.ch != '\0' {
                end = x + 1;
            }
        }
        if end > 0 {
            rows.push((y, end));
        }
    }

    let mut spans = Vec::<StyledSpan>::new();
    for (row_index, (y, end)) in rows.into_iter().enumerate() {
        if row_index > 0 {
            push_span(&mut spans, ' ', Default::default(), &screen);
        }
        for x in 0..end {
            let cell = screen.char_at(Position::new(x, y));
            push_span(&mut spans, if cell.ch == '\0' { ' ' } else { cell.ch }, cell.attribute, &screen);
        }
    }

    let plain = spans.iter().map(|span| span.text.as_str()).collect();
    let styled = spans.iter().any(has_style).then(|| spans.into_boxed_slice());
    HeaderText { plain, styled }
}

fn push_span(spans: &mut Vec<StyledSpan>, ch: char, attribute: icy_engine::TextAttribute, screen: &TextScreen) {
    let foreground = color(attribute.foreground_color(), screen, true);
    let background = color(attribute.background_color(), screen, false);
    let style = StyledSpan {
        text: ch.to_string(),
        foreground,
        background,
        bold: attribute.is_bold(),
        italic: attribute.is_italic(),
        underline: attribute.is_underlined(),
        strikethrough: attribute.is_crossed_out(),
    };
    if let Some(last) = spans.last_mut().filter(|last| same_style(last, &style)) {
        last.text.push(ch);
    } else {
        spans.push(style);
    }
}

fn color(color: AttributeColor, screen: &TextScreen, foreground: bool) -> Option<[u8; 3]> {
    match color {
        AttributeColor::Palette(index) if (foreground && index == 7) || (!foreground && index == 0) => None,
        AttributeColor::Palette(index) => {
            let (red, green, blue) = screen.palette().rgb(u32::from(index));
            Some([red, green, blue])
        }
        AttributeColor::ExtendedPalette(index) => {
            let (red, green, blue) = XTERM_256_PALETTE[index as usize].1.rgb();
            Some([red, green, blue])
        }
        AttributeColor::Rgb(red, green, blue) => Some([red, green, blue]),
        AttributeColor::Transparent => None,
    }
}

fn same_style(left: &StyledSpan, right: &StyledSpan) -> bool {
    left.foreground == right.foreground
        && left.background == right.background
        && left.bold == right.bold
        && left.italic == right.italic
        && left.underline == right.underline
        && left.strikethrough == right.strikethrough
}

fn has_style(span: &StyledSpan) -> bool {
    span.foreground.is_some() || span.background.is_some() || span.bold || span.italic || span.underline || span.strikethrough
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_matches_ignoring_case() {
        assert_eq!(find_ignore_case("Coffee coffee", "COF"), [0..3, 7..10]);
        assert_eq!(find_ignore_case("J\u{dc}rgen", "\u{fc}r"), vec![1..4]);
        assert_eq!(find_ignore_case("aaaa", "aa"), [0..2, 2..4]);
        assert!(find_ignore_case("Alice", "").is_empty());
        assert!(find_ignore_case("Al", "Alice").is_empty());
    }

    #[test]
    fn decodes_ascii_utf8_and_cp437() {
        assert_eq!(HeaderText::new(b"Alice"), "Alice");
        assert_eq!(HeaderText::new("Andr\u{e9}".as_bytes()), "Andr\u{e9}");
        assert_eq!(HeaderText::new(b"Andr\x82 \xb0\xdb"), "Andr\u{e9} \u{2591}\u{2588}");
    }

    #[test]
    fn parses_ansi_cursor_movement_and_colors() {
        let malformed = HeaderText::new(b"to the correct areas\r\x1b[A\x1b[33C\x1b[1;30m.\x1b[B\x1b[34D");
        assert_eq!(malformed.as_str(), "to the correct areas");

        let colored = HeaderText::new(b"\x1b[1;31mAlert");
        assert_eq!(colored.as_str(), "Alert");
        let spans = colored.styled().unwrap();
        assert!(spans[0].foreground.is_some());
    }
}
