use codepages::tables::{CP437_TO_UNICODE, UNICODE_TO_CP437};
use icy_parser_core::{
    ATARI_ST_TO_UNICODE, ATARI_TO_UNICODE, PETSCII_TO_UNICODE, UNICODE_TO_ATARI, UNICODE_TO_ATARI_ST, UNICODE_TO_PETSCII, UNICODE_TO_VIEWDATA,
    VIEWDATA_TO_UNICODE,
};

use crate::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BufferType {
    Unicode,
    CP437,
    Petscii,
    Atascii,
    Viewdata,
    /// The Atari ST's character set, used by its VT52 screen.
    AtariSt,
}

impl BufferType {
    pub fn blink_rate(&self) -> u64 {
        match self {
            BufferType::CP437 => 533,
            BufferType::Viewdata => 1000,
            BufferType::Petscii => 500,
            BufferType::Atascii => 333,
            _ => 533,
        }
    }
    pub fn caret_blink_rate(&self) -> u64 {
        match self {
            BufferType::CP437 => 266,
            BufferType::Viewdata => 500,
            BufferType::Petscii => 500,
            BufferType::Atascii => 333,
            _ => 266,
        }
    }
    pub fn from_byte(b: u8) -> Self {
        match b {
            // 0 => BufferType::Unicode,
            1 => BufferType::CP437,
            2 => BufferType::Petscii,
            3 => BufferType::Atascii,
            4 => BufferType::Viewdata,
            5 => BufferType::AtariSt,
            _ => BufferType::Unicode,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            BufferType::Unicode => 0,
            BufferType::CP437 => 1,
            BufferType::Petscii => 2,
            BufferType::Atascii => 3,
            BufferType::Viewdata => 4,
            BufferType::AtariSt => 5,
        }
    }

    pub fn selection_colors(&self) -> (Color, Color) {
        match self {
            // CP437 and Unicode use VGA-style magenta on gray selection
            BufferType::CP437 | BufferType::Unicode => (
                Color::new(0xAA, 0x00, 0xAA), // Magenta foreground
                Color::new(0xAA, 0xAA, 0xAA), // Gray background
            ),
            // Petscii uses Commodore VIC colors
            BufferType::Petscii => (
                Color::new(0x37, 0x39, 0xC4), // VIC blue foreground
                Color::new(0xB0, 0x3F, 0xB6), // VIC purple background
            ),
            // Atascii uses Atari ANTIC colors
            BufferType::Atascii => (
                Color::new(0x09, 0x51, 0x83), // ANTIC blue foreground
                Color::new(0xFF, 0xFF, 0xFF), // White background
            ),
            // Viewdata uses black on white like Videotex/Mode7, as does the Atari ST's desktop
            BufferType::Viewdata | BufferType::AtariSt => (
                Color::new(0x00, 0x00, 0x00), // Black foreground
                Color::new(0xFF, 0xFF, 0xFF), // White background
            ),
        }
    }

    pub fn convert_to_unicode(&self, ch: char) -> char {
        match self {
            BufferType::Unicode => ch, // Already Unicode, no conversion needed

            BufferType::CP437 => match CP437_TO_UNICODE.get(ch as usize) {
                Some(out_ch) => *out_ch,
                _ => ch,
            },

            BufferType::Petscii => {
                if let Some(tch) = PETSCII_TO_UNICODE.get(&(ch as u8)) {
                    *tch as char
                } else {
                    ch
                }
            }

            BufferType::Atascii => {
                // Use the ATASCII converter for Atari characters
                match ATARI_TO_UNICODE.get(ch as usize) {
                    Some(out_ch) => *out_ch,
                    _ => ch,
                }
            }

            BufferType::AtariSt => ATARI_ST_TO_UNICODE.get(ch as usize).copied().unwrap_or(ch),

            BufferType::Viewdata if ch as u32 == 0xA6 => '¦',
            BufferType::Viewdata if ch == '|' => '|',
            BufferType::Viewdata => match VIEWDATA_TO_UNICODE.get(ch as usize) {
                Some(out_ch) => *out_ch,
                _ => ch,
            },
        }
    }

    pub fn convert_from_unicode(&self, ch: char) -> char {
        match self {
            BufferType::Unicode => ch, // Already Unicode, no conversion needed

            BufferType::CP437 => {
                if let Some(tch) = UNICODE_TO_CP437.get(&ch) {
                    *tch as char
                } else {
                    ch
                }
            }

            BufferType::Petscii => {
                if let Some(tch) = UNICODE_TO_PETSCII.get(&(ch as u8)) {
                    *tch as char
                } else {
                    ch
                }
            }

            BufferType::Atascii => {
                // Use the ATASCII converter for Atari characters
                match UNICODE_TO_ATARI.get(&ch) {
                    Some(out_ch) => *out_ch,
                    _ => ch,
                }
            }

            BufferType::AtariSt => UNICODE_TO_ATARI_ST.get(&ch).copied().unwrap_or(ch),

            BufferType::Viewdata => {
                if ch == ' ' {
                    return ' ';
                }
                if ch == '¦' {
                    return '\u{00A6}';
                }
                if ch == '|' {
                    return '|';
                }
                match UNICODE_TO_VIEWDATA.get(&ch) {
                    Some(out_ch) => *out_ch,
                    // For Viewdata/Mode7, unknown characters should be filtered
                    // to prevent sending invalid bytes to the BBS.
                    // Return NUL character to indicate the character should be skipped.
                    _ => '\0',
                }
            }
        }
    }

    pub fn try_convert_from_unicode(&self, ch: char) -> Option<char> {
        match self {
            BufferType::Unicode => Some(ch), // Already Unicode, no conversion needed

            BufferType::CP437 => UNICODE_TO_CP437.get(&ch).map(|tch| *tch as char),

            BufferType::Petscii => UNICODE_TO_PETSCII.get(&(ch as u8)).map(|tch| *tch as char),

            BufferType::Atascii => {
                // Use the ATASCII converter for Atari characters
                UNICODE_TO_ATARI.get(&ch).copied()
            }

            BufferType::AtariSt => UNICODE_TO_ATARI_ST.get(&ch).copied(),

            BufferType::Viewdata => {
                if ch == ' ' {
                    return Some(' ');
                }
                UNICODE_TO_VIEWDATA.get(&ch).copied()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BufferType;

    #[test]
    fn atari_st_characters_convert_both_ways() {
        for (code, unicode) in [(0x41, 'A'), (0x84, 'ä'), (0x9E, 'ß'), (0xE0, 'α'), (0xBD, '©')] {
            assert_eq!(BufferType::AtariSt.convert_to_unicode(char::from(code)), unicode);
            assert_eq!(BufferType::AtariSt.convert_from_unicode(unicode) as u32, u32::from(code));
        }
        assert_eq!(BufferType::from_byte(BufferType::AtariSt.to_byte()), BufferType::AtariSt);
    }

    #[test]
    fn viewdata_distinguishes_broken_vertical_and_pipe() {
        assert_eq!(BufferType::Viewdata.convert_to_unicode('\u{00A6}'), '¦');
        assert_eq!(BufferType::Viewdata.convert_to_unicode('|'), '|');
        assert_eq!(BufferType::Viewdata.convert_from_unicode('¦') as u32, 0xA6);
        assert_eq!(BufferType::Viewdata.convert_from_unicode('|'), '|');
    }
}
