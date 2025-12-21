//! PSF font format parser (PSF1 and PSF2).
//!
//! This module provides direct PSF parsing without going through libyaff,
//! loading fonts directly into the compact glyph representation.
//!
//! PSF1 Format:
//! - 4 byte header: magic (0x36, 0x04), mode, charsize
//! - 256 or 512 glyphs (8 pixels wide)
//! - Optional Unicode table
//!
//! PSF2 Format:
//! - 32 byte header with magic, version, header size, flags, glyph count, char size, height, width
//! - Variable number of glyphs
//! - Optional Unicode table

use super::compact_glyph::CompactGlyph;
use crate::{EngineError, FontError};

/// PSF1 magic number (little-endian: 0x0436)
pub const PSF1_MAGIC: u16 = 0x0436;

/// PSF2 magic number (little-endian: 0x864AB572)
pub const PSF2_MAGIC: u32 = 0x864A_B572;

// PSF1 mode flags
const PSF1_MODE512: u8 = 0x01;
#[allow(dead_code)]
const PSF1_MODEHASTAB: u8 = 0x02;

// PSF2 flags
const PSF2_HAS_UNICODE_TABLE: u32 = 0x01;

/// Parsed PSF font data
#[derive(Debug, Clone)]
pub struct PsfFont {
    /// Font name (optional, not stored in PSF format)
    pub name: Option<String>,
    /// Glyph width in pixels
    pub width: u8,
    /// Glyph height in pixels
    pub height: u8,
    /// 256 glyphs (we only support the first 256 for our use case)
    pub glyphs: [CompactGlyph; 256],
}

impl Default for PsfFont {
    fn default() -> Self {
        Self {
            name: None,
            width: 8,
            height: 16,
            glyphs: [CompactGlyph::EMPTY; 256],
        }
    }
}

impl PsfFont {
    /// Parse a PSF font from bytes (auto-detects PSF1 or PSF2).
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        if bytes.len() < 4 {
            return Err(FontError::UnknownFontFormat(bytes.len()).into());
        }

        // Check PSF1 magic
        let magic16 = u16::from_le_bytes([bytes[0], bytes[1]]);
        if magic16 == PSF1_MAGIC {
            return Self::parse_psf1(bytes);
        }

        // Check PSF2 magic
        let magic32 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic32 == PSF2_MAGIC {
            return Self::parse_psf2(bytes);
        }

        Err(FontError::UnknownFontFormat(bytes.len()).into())
    }

    /// Parse a PSF1 format font.
    fn parse_psf1(bytes: &[u8]) -> crate::Result<Self> {
        if bytes.len() < 4 {
            return Err(EngineError::Generic("PSF1: file too short".into()));
        }

        let mode = bytes[2];
        let char_size = bytes[3] as usize;

        if char_size == 0 {
            return Err(EngineError::Generic("PSF1: zero charsize not allowed".into()));
        }

        let glyph_count = if mode & PSF1_MODE512 != 0 { 512 } else { 256 };
        let bitmap_start = 4;
        let bitmap_len = glyph_count * char_size;

        if bytes.len() < bitmap_start + bitmap_len {
            return Err(EngineError::Generic("PSF1: bitmap data truncated".into()));
        }

        let width = 8u8;
        let height = char_size as u8;

        let mut font = Self {
            name: None,
            width,
            height,
            glyphs: std::array::from_fn(|_| CompactGlyph::new(width, height)),
        };

        // Load glyphs (only first 256)
        for glyph_idx in 0..256.min(glyph_count) {
            let offset = bitmap_start + glyph_idx * char_size;
            let glyph_data = &bytes[offset..offset + char_size];
            font.glyphs[glyph_idx] = CompactGlyph::from_rows(width, height, glyph_data);
        }

        Ok(font)
    }

    /// Parse a PSF2 format font.
    fn parse_psf2(bytes: &[u8]) -> crate::Result<Self> {
        if bytes.len() < 32 {
            return Err(EngineError::Generic("PSF2: header too short".into()));
        }

        // Parse header
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version > 0 {
            return Err(EngineError::Generic(format!("PSF2: unsupported version {version}")));
        }

        let header_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let _flags = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let glyph_count = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
        let char_size = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as usize;
        let height = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]) as usize;
        let width = u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]) as usize;

        // Validate
        if width == 0 || height == 0 || char_size == 0 {
            return Err(EngineError::Generic("PSF2: invalid dimensions".into()));
        }

        let bytes_per_row = (width + 7) / 8;
        let expected_char_size = height * bytes_per_row;
        if expected_char_size != char_size {
            return Err(EngineError::Generic(format!(
                "PSF2: char_size mismatch, header {char_size}, computed {expected_char_size}"
            )));
        }

        let bitmap_start = header_size;
        let bitmap_len = glyph_count * char_size;

        if bytes.len() < bitmap_start + bitmap_len {
            return Err(EngineError::Generic("PSF2: bitmap data truncated".into()));
        }

        // We only support fonts up to 8 pixels wide
        if width > 8 {
            return Err(EngineError::Generic(format!("PSF2: font width {width} exceeds maximum of 8 pixels")));
        }

        let w = width as u8;
        let h = height.min(32) as u8;

        let mut font = Self {
            name: None,
            width: w,
            height: h,
            glyphs: std::array::from_fn(|_| CompactGlyph::new(w, h)),
        };

        // Load glyphs (only first 256)
        for glyph_idx in 0..256.min(glyph_count) {
            let offset = bitmap_start + glyph_idx * char_size;

            // Extract row data - PSF2 rows may span multiple bytes
            let mut row_data = [0u8; 32];
            for y in 0..h as usize {
                let row_offset = offset + y * bytes_per_row;
                // For fonts <= 8px wide, we only need the first byte
                row_data[y] = bytes[row_offset];
            }

            font.glyphs[glyph_idx] = CompactGlyph::from_rows(w, h, &row_data[..h as usize]);
        }

        Ok(font)
    }

    /// Create from raw bytes (simple format: just glyph data, 256 glyphs, 8px wide).
    pub fn from_raw_bytes(width: u8, height: u8, data: &[u8]) -> crate::Result<Self> {
        let bytes_per_glyph = height as usize;
        let expected_len = 256 * bytes_per_glyph;

        if data.len() < expected_len {
            return Err(EngineError::Generic(format!("Raw font: expected {} bytes, got {}", expected_len, data.len())));
        }

        let mut font = Self {
            name: None,
            width,
            height,
            glyphs: std::array::from_fn(|_| CompactGlyph::new(width, height)),
        };

        for glyph_idx in 0..256 {
            let offset = glyph_idx * bytes_per_glyph;
            let glyph_data = &data[offset..offset + bytes_per_glyph];
            font.glyphs[glyph_idx] = CompactGlyph::from_rows(width, height, glyph_data);
        }

        Ok(font)
    }

    /// Convert to PSF2 bytes.
    pub fn to_psf2_bytes(&self) -> Vec<u8> {
        let width = self.width as usize;
        let height = self.height as usize;
        let bytes_per_row = (width + 7) / 8;
        let char_size = height * bytes_per_row;

        let mut data = Vec::new();

        // Header (32 bytes)
        data.extend_from_slice(&PSF2_MAGIC.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()); // version
        data.extend_from_slice(&32u32.to_le_bytes()); // header_size
        data.extend_from_slice(&0u32.to_le_bytes()); // flags (no unicode table)
        data.extend_from_slice(&256u32.to_le_bytes()); // glyph count
        data.extend_from_slice(&(char_size as u32).to_le_bytes());
        data.extend_from_slice(&(height as u32).to_le_bytes());
        data.extend_from_slice(&(width as u32).to_le_bytes());

        // Glyph data
        for glyph in &self.glyphs {
            for y in 0..height {
                data.push(glyph.get_row(y));
            }
        }

        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psf1_parse() {
        // Create minimal PSF1 font: magic + mode + charsize + 256 blank glyphs
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PSF1_MAGIC.to_le_bytes());
        bytes.push(0); // mode
        bytes.push(8); // charsize (8 bytes per glyph = 8 rows)
        bytes.extend_from_slice(&vec![0u8; 256 * 8]);

        let font = PsfFont::from_bytes(&bytes).unwrap();
        assert_eq!(font.width, 8);
        assert_eq!(font.height, 8);
    }

    #[test]
    fn test_psf2_parse() {
        // Create minimal PSF2 font
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PSF2_MAGIC.to_le_bytes()); // magic
        bytes.extend_from_slice(&0u32.to_le_bytes()); // version
        bytes.extend_from_slice(&32u32.to_le_bytes()); // header_size
        bytes.extend_from_slice(&0u32.to_le_bytes()); // flags
        bytes.extend_from_slice(&256u32.to_le_bytes()); // glyph count
        bytes.extend_from_slice(&16u32.to_le_bytes()); // char_size
        bytes.extend_from_slice(&16u32.to_le_bytes()); // height
        bytes.extend_from_slice(&8u32.to_le_bytes()); // width

        // 256 glyphs * 16 bytes each
        bytes.extend_from_slice(&vec![0u8; 256 * 16]);

        let font = PsfFont::from_bytes(&bytes).unwrap();
        assert_eq!(font.width, 8);
        assert_eq!(font.height, 16);
    }

    #[test]
    fn test_psf2_roundtrip() {
        let mut font = PsfFont::default();
        font.width = 8;
        font.height = 16;

        // Set some pixels
        font.glyphs[65].set_pixel(0, 0, true); // 'A'
        font.glyphs[65].set_pixel(7, 15, true);

        let psf2_bytes = font.to_psf2_bytes();
        let loaded = PsfFont::from_bytes(&psf2_bytes).unwrap();

        assert_eq!(loaded.width, 8);
        assert_eq!(loaded.height, 16);
        assert!(loaded.glyphs[65].get_pixel(0, 0));
        assert!(loaded.glyphs[65].get_pixel(7, 15));
    }

    #[test]
    fn test_raw_bytes() {
        let data = vec![0xFFu8; 256 * 16];
        let font = PsfFont::from_raw_bytes(8, 16, &data).unwrap();

        assert_eq!(font.width, 8);
        assert_eq!(font.height, 16);

        // All pixels should be on
        for x in 0..8 {
            assert!(font.glyphs[0].get_pixel(x, 0));
        }
    }
}
