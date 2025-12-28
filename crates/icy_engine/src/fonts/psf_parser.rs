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
const PSF1_MODEHASTAB: u8 = 0x02;
const PSF1_STARTSEQ: u16 = 0xFFFE;
const PSF1_SEPARATOR: u16 = 0xFFFF;

// PSF2 flags
const PSF2_HAS_UNICODE_TABLE: u32 = 0x01;
const PSF2_SEPARATOR: u8 = 0xFF;
const PSF2_STARTSEQ: u8 = 0xFE;

/// Unicode mapping for a glyph - can be a single codepoint or a sequence
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnicodeMapping {
    /// Single Unicode codepoint
    Single(u32),
    /// Sequence of Unicode codepoints (for combining characters, ligatures, etc.)
    Sequence(Vec<u32>),
}

/// Parsed PSF font data
#[derive(Debug, Clone)]
pub struct PsfFont {
    /// Font name (optional, not stored in PSF format)
    pub name: Option<String>,
    /// Glyph width in pixels
    pub width: u8,
    /// Glyph height in pixels
    pub height: u8,
    /// All glyphs in the font (PSF1: 256 or 512, PSF2: any count)
    pub glyphs: Vec<CompactGlyph>,
    /// Unicode mappings for each glyph (glyph index -> list of Unicode mappings)
    pub unicode_table: Vec<Vec<UnicodeMapping>>,
}

impl Default for PsfFont {
    fn default() -> Self {
        Self {
            name: None,
            width: 8,
            height: 16,
            glyphs: vec![CompactGlyph::EMPTY; 256],
            unicode_table: Vec::new(),
        }
    }
}

impl PsfFont {
    /// Create a new PSF font with the specified dimensions and glyph count
    pub fn new(width: u8, height: u8, glyph_count: usize) -> Self {
        Self {
            name: None,
            width,
            height,
            glyphs: vec![CompactGlyph::new(width, height); glyph_count],
            unicode_table: Vec::new(),
        }
    }

    /// Get the number of glyphs in this font
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// Get a glyph by index, returns None if out of bounds
    pub fn get_glyph(&self, index: usize) -> Option<&CompactGlyph> {
        self.glyphs.get(index)
    }

    /// Get a mutable glyph by index, returns None if out of bounds
    pub fn get_glyph_mut(&mut self, index: usize) -> Option<&mut CompactGlyph> {
        self.glyphs.get_mut(index)
    }

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

    /// Check if this font has Unicode mappings
    pub fn has_unicode_table(&self) -> bool {
        !self.unicode_table.is_empty() && self.unicode_table.iter().any(|m| !m.is_empty())
    }

    /// Get Unicode mappings for a glyph index
    pub fn get_unicode_mappings(&self, glyph_idx: usize) -> Option<&[UnicodeMapping]> {
        self.unicode_table.get(glyph_idx).map(std::vec::Vec::as_slice)
    }

    /// Add a Unicode mapping for a glyph
    pub fn add_unicode_mapping(&mut self, glyph_idx: usize, mapping: UnicodeMapping) {
        // Ensure unicode_table is large enough
        if self.unicode_table.len() <= glyph_idx {
            self.unicode_table.resize(glyph_idx + 1, Vec::new());
        }
        self.unicode_table[glyph_idx].push(mapping);
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
            glyphs: vec![CompactGlyph::new(width, height); glyph_count],
            unicode_table: Vec::new(),
        };

        // Load all glyphs
        for glyph_idx in 0..glyph_count {
            let offset = bitmap_start + glyph_idx * char_size;
            let glyph_data = &bytes[offset..offset + char_size];
            font.glyphs[glyph_idx] = CompactGlyph::from_rows(width, height, glyph_data);
        }

        // Parse Unicode table if present
        if mode & PSF1_MODEHASTAB != 0 {
            let unicode_start = bitmap_start + bitmap_len;
            if unicode_start < bytes.len() {
                Self::parse_psf1_unicode_table(&mut font, &bytes[unicode_start..], glyph_count);
            }
        }

        Ok(font)
    }

    /// Parse PSF1 Unicode table (16-bit code units).
    fn parse_psf1_unicode_table(font: &mut PsfFont, table_data: &[u8], glyph_count: usize) {
        let mut pos = 0;
        let mut glyph_idx = 0;

        // Ensure unicode_table is large enough
        font.unicode_table.resize(glyph_count, Vec::new());

        while glyph_idx < glyph_count && pos + 2 <= table_data.len() {
            let mut current_sequence: Option<Vec<u32>> = None;

            // Parse entries for this glyph until we hit separator
            while pos + 2 <= table_data.len() {
                let val = u16::from_le_bytes([table_data[pos], table_data[pos + 1]]);
                pos += 2;

                if val == PSF1_SEPARATOR {
                    // End of glyph description
                    if let Some(seq) = current_sequence.take() {
                        if !seq.is_empty() {
                            font.unicode_table[glyph_idx].push(UnicodeMapping::Sequence(seq));
                        }
                    }
                    break;
                } else if val == PSF1_STARTSEQ {
                    // Start of a new sequence
                    if let Some(seq) = current_sequence.take() {
                        if !seq.is_empty() {
                            font.unicode_table[glyph_idx].push(UnicodeMapping::Sequence(seq));
                        }
                    }
                    current_sequence = Some(Vec::new());
                } else {
                    // Regular codepoint
                    let cp = val as u32;
                    if let Some(seq) = current_sequence.as_mut() {
                        seq.push(cp);
                    } else {
                        font.unicode_table[glyph_idx].push(UnicodeMapping::Single(cp));
                    }
                }
            }

            glyph_idx += 1;
        }
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
        let flags = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let glyph_count = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
        let char_size = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as usize;
        let height = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]) as usize;
        let width = u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]) as usize;

        // Validate
        if width == 0 || height == 0 || char_size == 0 {
            return Err(EngineError::Generic("PSF2: invalid dimensions".into()));
        }

        let bytes_per_row = width.div_ceil(8);
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
            glyphs: vec![CompactGlyph::new(w, h); glyph_count],
            unicode_table: Vec::new(),
        };

        // Load all glyphs
        for glyph_idx in 0..glyph_count {
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

        // Parse Unicode table if present
        if flags & PSF2_HAS_UNICODE_TABLE != 0 {
            let unicode_start = bitmap_start + bitmap_len;
            if unicode_start < bytes.len() {
                Self::parse_psf2_unicode_table(&mut font, &bytes[unicode_start..], glyph_count);
            }
        }

        Ok(font)
    }

    /// Parse PSF2 Unicode table (UTF-8 encoded).
    fn parse_psf2_unicode_table(font: &mut PsfFont, table_data: &[u8], glyph_count: usize) {
        let mut pos = 0;
        let mut glyph_idx = 0;

        // Ensure unicode_table is large enough
        font.unicode_table.resize(glyph_count, Vec::new());

        while pos < table_data.len() && glyph_idx < glyph_count {
            let mut current_sequence: Option<Vec<u32>> = None;

            // Parse until we hit a terminator (0xFF)
            while pos < table_data.len() {
                let byte = table_data[pos];

                if byte == PSF2_SEPARATOR {
                    // Terminator - end of this glyph's Unicode description
                    if let Some(seq) = current_sequence.take() {
                        if !seq.is_empty() {
                            font.unicode_table[glyph_idx].push(UnicodeMapping::Sequence(seq));
                        }
                    }
                    pos += 1;
                    break;
                } else if byte == PSF2_STARTSEQ {
                    // Start of a sequence
                    if let Some(seq) = current_sequence.take() {
                        if !seq.is_empty() {
                            font.unicode_table[glyph_idx].push(UnicodeMapping::Sequence(seq));
                        }
                    }
                    current_sequence = Some(Vec::new());
                    pos += 1;
                } else {
                    // Parse UTF-8 encoded Unicode value
                    if let Some((codepoint, bytes_read)) = Self::parse_utf8_char(&table_data[pos..]) {
                        if let Some(seq) = current_sequence.as_mut() {
                            seq.push(codepoint);
                        } else {
                            font.unicode_table[glyph_idx].push(UnicodeMapping::Single(codepoint));
                        }
                        pos += bytes_read;
                    } else {
                        // Invalid UTF-8, skip byte
                        pos += 1;
                    }
                }
            }

            glyph_idx += 1;
        }
    }

    /// Parse a UTF-8 encoded character from bytes.
    /// Returns (codepoint, `bytes_consumed`) or None if invalid.
    fn parse_utf8_char(bytes: &[u8]) -> Option<(u32, usize)> {
        if bytes.is_empty() {
            return None;
        }

        let first = bytes[0];

        // Single byte (0xxxxxxx)
        if first & 0x80 == 0 {
            return Some((first as u32, 1));
        }

        // Two bytes (110xxxxx 10xxxxxx)
        if first & 0xE0 == 0xC0 {
            if bytes.len() < 2 {
                return None;
            }
            let codepoint = ((first & 0x1F) as u32) << 6 | ((bytes[1] & 0x3F) as u32);
            return Some((codepoint, 2));
        }

        // Three bytes (1110xxxx 10xxxxxx 10xxxxxx)
        if first & 0xF0 == 0xE0 {
            if bytes.len() < 3 {
                return None;
            }
            let codepoint = ((first & 0x0F) as u32) << 12 | ((bytes[1] & 0x3F) as u32) << 6 | ((bytes[2] & 0x3F) as u32);
            return Some((codepoint, 3));
        }

        // Four bytes (11110xxx 10xxxxxx 10xxxxxx 10xxxxxx)
        if first & 0xF8 == 0xF0 {
            if bytes.len() < 4 {
                return None;
            }
            let codepoint = ((first & 0x07) as u32) << 18 | ((bytes[1] & 0x3F) as u32) << 12 | ((bytes[2] & 0x3F) as u32) << 6 | ((bytes[3] & 0x3F) as u32);
            return Some((codepoint, 4));
        }

        None
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
            glyphs: vec![CompactGlyph::new(width, height); 256],
            unicode_table: Vec::new(),
        };

        for glyph_idx in 0..256 {
            let offset = glyph_idx * bytes_per_glyph;
            let glyph_data = &data[offset..offset + bytes_per_glyph];
            font.glyphs[glyph_idx] = CompactGlyph::from_rows(width, height, glyph_data);
        }

        Ok(font)
    }

    /// Convert to PSF2 bytes with optional Unicode table.
    pub fn to_psf2_bytes(&self) -> Vec<u8> {
        let width = self.width as usize;
        let height = self.height as usize;
        let bytes_per_row = width.div_ceil(8);
        let char_size = height * bytes_per_row;
        let glyph_count = self.glyphs.len();

        let has_unicode = self.has_unicode_table();
        let flags = if has_unicode { PSF2_HAS_UNICODE_TABLE } else { 0 };

        let mut data = Vec::new();

        // Header (32 bytes)
        data.extend_from_slice(&PSF2_MAGIC.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()); // version
        data.extend_from_slice(&32u32.to_le_bytes()); // header_size
        data.extend_from_slice(&flags.to_le_bytes()); // flags
        data.extend_from_slice(&(glyph_count as u32).to_le_bytes()); // glyph count
        data.extend_from_slice(&(char_size as u32).to_le_bytes());
        data.extend_from_slice(&(height as u32).to_le_bytes());
        data.extend_from_slice(&(width as u32).to_le_bytes());

        // Glyph data
        for glyph in &self.glyphs {
            for y in 0..height {
                data.push(glyph.get_row(y));
            }
        }

        // Unicode table
        if has_unicode {
            for glyph_idx in 0..glyph_count {
                if let Some(mappings) = self.unicode_table.get(glyph_idx) {
                    for mapping in mappings {
                        match mapping {
                            UnicodeMapping::Single(cp) => {
                                Self::write_utf8_codepoint(&mut data, *cp);
                            }
                            UnicodeMapping::Sequence(cps) => {
                                if cps.len() == 1 {
                                    Self::write_utf8_codepoint(&mut data, cps[0]);
                                } else {
                                    data.push(PSF2_STARTSEQ);
                                    for cp in cps {
                                        Self::write_utf8_codepoint(&mut data, *cp);
                                    }
                                }
                            }
                        }
                    }
                }
                data.push(PSF2_SEPARATOR);
            }
        }

        data
    }

    /// Write a Unicode codepoint as UTF-8 bytes
    fn write_utf8_codepoint(data: &mut Vec<u8>, codepoint: u32) {
        if codepoint < 0x80 {
            // Single byte
            data.push(codepoint as u8);
        } else if codepoint < 0x800 {
            // Two bytes
            data.push(0xC0 | ((codepoint >> 6) as u8));
            data.push(0x80 | ((codepoint & 0x3F) as u8));
        } else if codepoint < 0x10000 {
            // Three bytes
            data.push(0xE0 | ((codepoint >> 12) as u8));
            data.push(0x80 | (((codepoint >> 6) & 0x3F) as u8));
            data.push(0x80 | ((codepoint & 0x3F) as u8));
        } else if codepoint < 0x110000 {
            // Four bytes
            data.push(0xF0 | ((codepoint >> 18) as u8));
            data.push(0x80 | (((codepoint >> 12) & 0x3F) as u8));
            data.push(0x80 | (((codepoint >> 6) & 0x3F) as u8));
            data.push(0x80 | ((codepoint & 0x3F) as u8));
        }
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
    fn test_psf2_unicode_roundtrip() {
        let mut font = PsfFont::default();
        font.width = 8;
        font.height = 16;

        // Add Unicode mappings
        font.add_unicode_mapping(65, UnicodeMapping::Single(0x41)); // 'A'
        font.add_unicode_mapping(65, UnicodeMapping::Single(0x0391)); // Greek Alpha
        font.add_unicode_mapping(66, UnicodeMapping::Single(0x42)); // 'B'
        font.add_unicode_mapping(67, UnicodeMapping::Sequence(vec![0x63, 0x0327])); // 'c' + combining cedilla

        let psf2_bytes = font.to_psf2_bytes();
        let loaded = PsfFont::from_bytes(&psf2_bytes).unwrap();

        assert!(loaded.has_unicode_table());

        // Check glyph 65 has two mappings
        let mappings_65 = loaded.get_unicode_mappings(65).unwrap();
        assert_eq!(mappings_65.len(), 2);
        assert_eq!(mappings_65[0], UnicodeMapping::Single(0x41));
        assert_eq!(mappings_65[1], UnicodeMapping::Single(0x0391));

        // Check glyph 66
        let mappings_66 = loaded.get_unicode_mappings(66).unwrap();
        assert_eq!(mappings_66.len(), 1);
        assert_eq!(mappings_66[0], UnicodeMapping::Single(0x42));

        // Check glyph 67 (sequence)
        let mappings_67 = loaded.get_unicode_mappings(67).unwrap();
        assert_eq!(mappings_67.len(), 1);
        assert_eq!(mappings_67[0], UnicodeMapping::Sequence(vec![0x63, 0x0327]));
    }

    #[test]
    fn test_utf8_encoding() {
        // Test various codepoints encode/decode correctly
        let test_cases = [
            (0x41, 1),    // ASCII 'A'
            (0x00E9, 2),  // é (Latin Small Letter E with Acute)
            (0x4E2D, 3),  // 中 (CJK Unified Ideograph)
            (0x1F600, 4), // 😀 (Grinning Face emoji)
        ];

        for (codepoint, expected_len) in test_cases {
            let mut data = Vec::new();
            PsfFont::write_utf8_codepoint(&mut data, codepoint);
            assert_eq!(data.len(), expected_len, "Wrong length for U+{:04X}", codepoint);

            let (decoded, len) = PsfFont::parse_utf8_char(&data).unwrap();
            assert_eq!(decoded, codepoint, "Roundtrip failed for U+{:04X}", codepoint);
            assert_eq!(len, expected_len);
        }
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

    #[test]
    fn test_psf1_512_glyphs() {
        // Create PSF1 font with 512 glyphs
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PSF1_MAGIC.to_le_bytes());
        bytes.push(PSF1_MODE512); // mode: 512 glyphs
        bytes.push(8); // charsize (8 bytes per glyph)

        // 512 glyphs * 8 bytes each
        for i in 0..512 {
            // Put unique pattern in each glyph
            for _row in 0..8 {
                bytes.push((i & 0xFF) as u8);
            }
        }

        let font = PsfFont::from_bytes(&bytes).unwrap();
        assert_eq!(font.width, 8);
        assert_eq!(font.height, 8);
        assert_eq!(font.glyph_count(), 512);

        // Check glyph 300 has the right pattern
        assert_eq!(font.glyphs[300].get_row(0), (300 & 0xFF) as u8);
    }

    #[test]
    fn test_psf2_custom_glyph_count() {
        // Create PSF2 font with 400 glyphs
        let glyph_count = 400usize;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PSF2_MAGIC.to_le_bytes()); // magic
        bytes.extend_from_slice(&0u32.to_le_bytes()); // version
        bytes.extend_from_slice(&32u32.to_le_bytes()); // header_size
        bytes.extend_from_slice(&0u32.to_le_bytes()); // flags (no unicode)
        bytes.extend_from_slice(&(glyph_count as u32).to_le_bytes()); // glyph count
        bytes.extend_from_slice(&16u32.to_le_bytes()); // char_size (16 bytes)
        bytes.extend_from_slice(&16u32.to_le_bytes()); // height
        bytes.extend_from_slice(&8u32.to_le_bytes()); // width

        // 400 glyphs * 16 bytes each
        for i in 0..glyph_count {
            for _row in 0..16 {
                bytes.push((i & 0xFF) as u8);
            }
        }

        let font = PsfFont::from_bytes(&bytes).unwrap();
        assert_eq!(font.width, 8);
        assert_eq!(font.height, 16);
        assert_eq!(font.glyph_count(), 400);

        // Check glyph 350 has the right pattern
        assert_eq!(font.glyphs[350].get_row(0), (350 & 0xFF) as u8);
    }

    #[test]
    fn test_psf2_roundtrip_many_glyphs() {
        // Create a font with 512 glyphs
        let mut font = PsfFont::new(8, 16, 512);

        // Set unique patterns
        for i in 0..512 {
            font.glyphs[i].set_pixel(i % 8, 0, true);
        }

        // Add unicode mapping for glyph 300
        font.add_unicode_mapping(300, UnicodeMapping::Single(0x1234));

        let psf2_bytes = font.to_psf2_bytes();
        let loaded = PsfFont::from_bytes(&psf2_bytes).unwrap();

        assert_eq!(loaded.glyph_count(), 512);
        assert_eq!(loaded.width, 8);
        assert_eq!(loaded.height, 16);

        // Verify glyph patterns
        for i in 0..512 {
            assert!(loaded.glyphs[i].get_pixel(i % 8, 0), "Glyph {} pixel mismatch", i);
        }

        // Check unicode mapping
        let mappings = loaded.get_unicode_mappings(300).unwrap();
        assert_eq!(mappings[0], UnicodeMapping::Single(0x1234));
    }
}
