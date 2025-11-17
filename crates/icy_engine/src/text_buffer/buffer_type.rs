use codepages::tables::*;
use icy_parser_core::*;

use crate::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BufferType {
    Unicode,
    CP437,
    Petscii,
    Atascii,
    Viewdata,
}

impl BufferType {
    pub fn from_byte(b: u8) -> Self {
        match b {
            // 0 => BufferType::Unicode,
            1 => BufferType::CP437,
            2 => BufferType::Petscii,
            3 => BufferType::Atascii,
            4 => BufferType::Viewdata,
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
        }
    }

    pub fn get_selection_colors(&self) -> (Color, Color) {
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
            // Viewdata uses black on white like Videotex/Mode7
            BufferType::Viewdata => (
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

            BufferType::Viewdata => {
                if ch == ' ' {
                    return ' ';
                }
                match UNICODE_TO_VIEWDATA.get(&ch) {
                    Some(out_ch) => *out_ch,
                    _ => ch,
                }
            }
        }
    }
}
