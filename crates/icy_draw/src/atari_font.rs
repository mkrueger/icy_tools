//! Fonts for ATASCII screens.
//!
//! An Atari font has 128 characters of 8 × 8 pixels; the screen shows the upper 128 codes as
//! their inverse. Raw font files (.fnt) keep the characters in the order of the Atari's screen
//! memory rather than in ATASCII order.

use icy_engine::{BitFont, Size};

use crate::fl;

const GLYPH_BYTES: usize = 8;
const RAW_SIZE: usize = 128 * GLYPH_BYTES;
/// Atari DOS binary files start with $FFFF and the start and end address.
const BINARY_HEADER: usize = 6;

/// The ATASCII code of the character at `internal` in screen memory order: punctuation and
/// digits, capitals, graphics, then lower case.
fn atascii_code(internal: usize) -> usize {
    match internal {
        0..=63 => internal + 32,
        64..=95 => internal - 64,
        _ => internal,
    }
}

/// Fills the upper half of a 256 character font with the inverse of the lower half.
fn invert_upper_half(bytes: &mut [u8; 256 * GLYPH_BYTES]) {
    let (lower, upper) = bytes.split_at_mut(RAW_SIZE);
    for (inverse, normal) in upper.iter_mut().zip(lower.iter()) {
        *inverse = !normal;
    }
}

/// Loads a font for ATASCII screens: a raw Atari font, also as an Atari DOS binary file, or an
/// 8 × 8 font in ATASCII order such as PSF. The inverse characters are made when missing.
pub fn load(name: &str, data: &[u8]) -> Result<BitFont, String> {
    let raw = match data.len() {
        RAW_SIZE => Some(data),
        size if size == RAW_SIZE + BINARY_HEADER && data.starts_with(&[0xFF, 0xFF]) => Some(&data[BINARY_HEADER..]),
        _ => None,
    };
    let mut bytes = [0u8; 256 * GLYPH_BYTES];
    if let Some(raw) = raw {
        for (internal, glyph) in raw.chunks_exact(GLYPH_BYTES).enumerate() {
            let code = atascii_code(internal);
            bytes[code * GLYPH_BYTES..(code + 1) * GLYPH_BYTES].copy_from_slice(glyph);
        }
        invert_upper_half(&mut bytes);
    } else {
        let font = BitFont::from_bytes(name, data).map_err(|error| error.to_string())?;
        if font.size() != Size::new(8, 8) {
            let size = font.size();
            return Err(fl!("atascii-font-size", width = size.width, height = size.height));
        }
        for code in 0..256u32 {
            let glyph = font.glyph(char::from_u32(code).unwrap_or(' '));
            let start = code as usize * GLYPH_BYTES;
            bytes[start..start + GLYPH_BYTES].copy_from_slice(&glyph.data[..GLYPH_BYTES]);
        }
        if bytes[RAW_SIZE..].iter().all(|&row| row == 0) {
            invert_upper_half(&mut bytes);
        }
    }
    Ok(BitFont::create_8(name, 8, 8, &bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A raw font whose character at screen memory position `i` has `i` in each row.
    fn raw_font() -> Vec<u8> {
        (0..128u8).flat_map(|internal| [internal; GLYPH_BYTES]).collect()
    }

    fn row(font: &BitFont, code: u32) -> u8 {
        font.glyph(char::from_u32(code).unwrap()).data[0]
    }

    #[test]
    fn raw_fonts_are_put_in_atascii_order_with_their_inverse() {
        let font = load("test", &raw_font()).unwrap();
        assert_eq!(font.size(), Size::new(8, 8));
        // Space is the first character in screen memory, the heart graphic the 65th.
        assert_eq!(row(&font, u32::from(b' ')), 0);
        assert_eq!(row(&font, u32::from(b'A')), 33);
        assert_eq!(row(&font, 0x00), 64);
        assert_eq!(row(&font, u32::from(b'a')), 97);
        assert_eq!(row(&font, 0x80 | u32::from(b'A')), !33);
    }

    #[test]
    fn atari_dos_binaries_are_raw_fonts_after_their_header() {
        let mut data = vec![0xFF, 0xFF, 0x00, 0xE0, 0xFF, 0xE3];
        data.extend(raw_font());
        assert_eq!(row(&load("test", &data).unwrap(), u32::from(b'A')), 33);
    }

    #[test]
    fn psf_fonts_keep_their_order_and_other_sizes_are_refused() {
        let atari = icy_engine::ATARI.clone();
        let font = load("atari", &include_bytes!("../../icy_engine/data/fonts/Atari/Atari_ATASCII.psf")[..]).unwrap();
        for code in [0x00, 0x41, 0xC1] {
            assert_eq!(
                font.glyph(char::from_u32(code).unwrap()).data[..8],
                atari.glyph(char::from_u32(code).unwrap()).data[..8]
            );
        }
        assert!(load("tall", &include_bytes!("../../icy_engine/data/fonts/Atari/atari-st-8x16.psf")[..]).is_err());
    }
}
