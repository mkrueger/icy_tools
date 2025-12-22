use base64::{Engine, engine::general_purpose};
#[allow(unused_imports)]
use lazy_static::lazy_static;
use libyaff::YaffFont;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{EngineError, FontError};
use std::{path::PathBuf, str::FromStr};

pub mod ansi;
pub mod compact_glyph;
pub mod legacy;
pub mod psf_parser;
pub mod rip;
pub mod sauce;
pub mod skypix;

pub use compact_glyph::CompactGlyph;
pub use psf_parser::PsfFont;

use super::Size;

// Re-export key items from submodules
pub use ansi::{ANSI_SLOT_COUNT, ANSI_SLOT_FONTS, CP437, DEFAULT_FONT_NAME, font_height_for_lines, get_ansi_font};
pub use legacy::{ATARI, ATARI_XEP80, ATARI_XEP80_INT, C64_SHIFTED, C64_UNSHIFTED, VIEWDATA};
pub use sauce::{SAUCE_FONT_MAP, get_sauce_font_names, load_sauce_font};
// Re-export byte data with short names for screen_modes compatibility
pub use skypix::get_amiga_font_by_name;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BitFontType {
    BuiltIn,
    Library,
    Custom,
}

#[derive(Debug, Clone)]
pub struct BitFont {
    /// Font name
    pub name: String,
    /// Width of each glyph in pixels (max 8)
    pub width: u8,
    /// Height of each glyph in pixels (max 32)
    pub height: u8,
    /// All 256 glyphs stored as compact bitmaps
    pub glyphs: [CompactGlyph; 256],
    /// Optional file path for custom fonts
    pub path_opt: Option<PathBuf>,
    /// Font type (built-in, library, or custom)
    pub font_type: BitFontType,
}

/// Serializable representation of BitFont for serde
#[derive(Serialize, Deserialize)]
struct BitFontSerde {
    name: String,
    width: u8,
    height: u8,
    data: Vec<u8>,
    font_type: BitFontType,
    path: Option<PathBuf>,
}

impl Serialize for BitFont {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        BitFontSerde {
            name: self.name.clone(),
            width: self.width,
            height: self.height,
            data: self.convert_to_u8_data(),
            font_type: self.font_type,
            path: self.path_opt.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BitFont {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let serde_font = BitFontSerde::deserialize(deserializer)?;
        let mut font = BitFont::create_8(&serde_font.name, serde_font.width, serde_font.height, &serde_font.data);
        font.font_type = serde_font.font_type;
        font.path_opt = serde_font.path;
        Ok(font)
    }
}

impl PartialEq for BitFont {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.width == other.width && self.height == other.height && self.glyphs == other.glyphs
    }
}

impl Default for BitFont {
    fn default() -> Self {
        BitFont::from_sauce_name("IBM VGA").unwrap()
    }
}

impl BitFont {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub fn size(&self) -> Size {
        Size::new(self.width as i32, self.height as i32)
    }

    pub fn font_type(&self) -> BitFontType {
        self.font_type
    }

    pub fn is_default(&self) -> bool {
        self.name() == ansi::DEFAULT_FONT_NAME || self.name() == ansi::ALT_DEFAULT_FONT_NAME
    }

    /// Get a glyph for the given character (0-255)
    /// Returns an empty glyph for characters outside the valid range
    #[inline]
    pub fn glyph(&self, ch: char) -> &CompactGlyph {
        let ch_code = ch as usize;
        if ch_code < 256 {
            &self.glyphs[ch_code]
        } else {
            // Return empty glyph for out-of-range characters
            &self.glyphs[0]
        }
    }

    /// Get mutable reference to a glyph
    #[inline]
    pub fn glyph_mut(&mut self, ch: char) -> &mut CompactGlyph {
        let ch_code = (ch as usize).min(255);
        &mut self.glyphs[ch_code]
    }

    /// Get reference to all glyphs
    pub fn glyphs(&self) -> &[CompactGlyph; 256] {
        &self.glyphs
    }

    /// Convert font to raw u8 data for legacy formats
    pub fn convert_to_u8_data(&self) -> Vec<u8> {
        let mut result = Vec::new();
        let height = self.height as usize;

        for glyph in &self.glyphs {
            // Copy the glyph data, padded/truncated to font height
            for y in 0..height {
                if y < 32 {
                    result.push(glyph.data[y]);
                } else {
                    result.push(0);
                }
            }
        }
        result
    }

    pub fn encode_as_ansi(&self, font_slot: usize) -> String {
        let font_data = self.convert_to_u8_data();
        let data = general_purpose::STANDARD.encode(font_data);
        format!("\x1BPCTerm:Font:{font_slot}:{data}\x1B\\")
    }

    /// Convert BitFont to YaffFont for export
    pub fn to_yaff_font(&self) -> YaffFont {
        use libyaff::{Bitmap, GlyphDefinition, Label};

        let mut glyphs = Vec::with_capacity(256);

        for i in 0..256u32 {
            let glyph = &self.glyphs[i as usize];
            let bitmap_pixels = glyph.to_bitmap_pixels();

            let glyph_def = GlyphDefinition {
                labels: vec![Label::Codepoint(vec![i as u16])],
                bitmap: Bitmap {
                    pixels: bitmap_pixels,
                    width: glyph.width as usize,
                    height: glyph.height as usize,
                },
                ..Default::default()
            };
            glyphs.push(glyph_def);
        }

        YaffFont {
            name: Some(self.name.clone()),
            pixel_size: Some(self.height as i32),
            line_height: Some(self.height as i32),
            bounding_box: Some((self.width as u32, self.height as u32)),
            cell_size: Some((self.width as u32, self.height as u32)),
            glyphs,
            ..Default::default()
        }
    }

    /// Create a font from raw 8-bit data
    pub fn create_8(name: impl Into<String>, width: u8, height: u8, data: &[u8]) -> Self {
        let height = height.min(32);
        let width = width.min(8);
        let name = name.into();

        let mut glyphs: [CompactGlyph; 256] = std::array::from_fn(|_| CompactGlyph::new(width, height));
        let bytes_per_glyph = height as usize;

        for i in 0..256 {
            let offset = i * bytes_per_glyph;
            if offset + bytes_per_glyph <= data.len() {
                let glyph = &mut glyphs[i];
                for y in 0..height as usize {
                    glyph.data[y] = data[offset + y];
                }
            }
        }

        Self {
            name,
            width,
            height,
            glyphs,
            path_opt: None,
            font_type: BitFontType::Custom,
        }
    }

    /// Alias for create_8 for compatibility
    pub fn from_basic(width: u8, height: u8, data: &[u8]) -> Self {
        Self::create_8("Custom", width, height, data)
    }

    /// Length field for compatibility (always 256 for standard fonts)
    pub fn length(&self) -> usize {
        256
    }

    /// Convert to PSF2 bytes format
    pub fn to_psf2_bytes(&self) -> crate::Result<Vec<u8>> {
        let psf = PsfFont {
            name: Some(self.name.clone()),
            glyphs: self.glyphs.to_vec(),
            width: self.width,
            height: self.height,
            unicode_table: Vec::new(),
        };
        Ok(psf.to_psf2_bytes())
    }

    /// Clone this font with a different line height using Atari-style row replication.
    /// This uses a Bresenham-like error accumulator so small size changes (e.g. 8->9)
    /// keep consistent baseline & stroke thickness, closer to original VDI behaviour.
    pub fn scale_to_height(&self, new_height: i32) -> crate::Result<Self> {
        if new_height == self.height as i32 {
            return Ok(self.clone());
        }

        let old_height = self.height.max(1) as usize;
        let target_height = (new_height.max(1) as usize).min(32);

        let mut new_glyphs: [CompactGlyph; 256] = std::array::from_fn(|_| CompactGlyph::new(self.width, target_height as u8));

        for (i, old_glyph) in self.glyphs.iter().enumerate() {
            let new_glyph = &mut new_glyphs[i];
            let mut err: isize = 0;
            let mut src_row: usize = 0;

            for y in 0..target_height {
                // Copy row from source
                if src_row < old_height {
                    new_glyph.data[y] = old_glyph.data[src_row];
                }
                err += old_height as isize;
                if err >= target_height as isize {
                    err -= target_height as isize;
                    src_row = (src_row + 1).min(old_height.saturating_sub(1));
                }
            }
        }

        Ok(Self {
            name: self.name.clone(),
            width: self.width,
            height: target_height as u8,
            glyphs: new_glyphs,
            path_opt: None,
            font_type: self.font_type,
        })
    }
}

impl BitFont {
    /// Load font from bytes (PSF1, PSF2, YAFF, or plain format)
    pub fn from_bytes(name: impl Into<String>, data: &[u8]) -> crate::Result<Self> {
        let name = name.into();

        // Try to parse as PSF font first
        if let Ok(psf) = PsfFont::from_bytes(data) {
            // Convert Vec<CompactGlyph> to [CompactGlyph; 256], taking first 256 glyphs
            let mut glyphs: [CompactGlyph; 256] = std::array::from_fn(|_| CompactGlyph::new(psf.width, psf.height));
            for (i, glyph) in psf.glyphs.iter().take(256).enumerate() {
                glyphs[i] = glyph.clone();
            }
            return Ok(Self {
                name,
                width: psf.width,
                height: psf.height,
                glyphs,
                path_opt: None,
                font_type: BitFontType::BuiltIn,
            });
        }

        // Try as YAFF format
        if let Ok(yaff) = YaffFont::from_bytes(data) {
            return Ok(Self::from_yaff_font(&yaff, name));
        }

        // Try as raw font data (must be multiple of 256)
        if data.len() % 256 == 0 && !data.is_empty() {
            let char_height = data.len() / 256;
            return Ok(Self::create_8(name, 8, char_height as u8, data));
        }

        Err(FontError::UnknownFontFormat(data.len()).into())
    }

    /// Load font from ANSI font slot (0-42)
    ///
    /// # Arguments
    /// * `font_page` - Font slot number (0-42)
    /// * `font_height` - Desired font height in pixels (8, 14, or 16)
    ///
    /// # Returns
    /// * `Ok(BitFont)` - The loaded font (cached for performance)
    /// * `Err` - If the slot is invalid
    pub fn from_ansi_font_page(font_page: u8, font_height: u8) -> Option<&'static Self> {
        ansi::get_ansi_font(font_page, font_height)
    }

    /// Load font from SAUCE font name
    ///
    /// # Arguments
    /// * `sauce_name` - SAUCE font name (e.g., "IBM VGA", "Amiga Topaz 1")
    ///
    /// # Returns
    /// * `Ok(BitFont)` - The loaded font
    /// * `Err` - If the font name is not supported
    pub fn from_sauce_name(sauce_name: &str) -> crate::Result<Self> {
        sauce::load_sauce_font(sauce_name)
    }

    /// Convert a YaffFont to BitFont (for loading YAFF format files)
    pub fn from_yaff_font(yaff_font: &YaffFont, name: impl Into<String>) -> Self {
        use libyaff::Label;

        // Determine font dimensions
        let width = yaff_font
            .bounding_box
            .map(|(w, _)| w as u8)
            .or(yaff_font.cell_size.map(|(w, _)| w as u8))
            .unwrap_or(8)
            .min(8);

        let height = yaff_font
            .pixel_size
            .or(yaff_font.size)
            .or(yaff_font.line_height)
            .or(yaff_font.bounding_box.map(|(_, h)| h as i32))
            .or(yaff_font.cell_size.map(|(_, h)| h as i32))
            .unwrap_or(16) as u8;
        let height = height.min(32);

        let mut glyphs: [CompactGlyph; 256] = std::array::from_fn(|_| CompactGlyph::new(width, height));

        // First pass: Fill glyphs with codepoint labels (highest priority)
        for glyph_def in &yaff_font.glyphs {
            for label in &glyph_def.labels {
                if let Label::Codepoint(codes) = label {
                    for &code in codes {
                        if (code as usize) < 256 {
                            let target = &mut glyphs[code as usize];
                            // Convert bitmap to CompactGlyph
                            for (y, row) in glyph_def.bitmap.pixels.iter().enumerate() {
                                if y >= 32 {
                                    break;
                                }
                                let mut packed: u8 = 0;
                                for (x, &pixel) in row.iter().enumerate() {
                                    if x >= 8 {
                                        break;
                                    }
                                    if pixel {
                                        packed |= 1 << (7 - x);
                                    }
                                }
                                target.data[y] = packed;
                            }
                        }
                    }
                }
            }
        }

        // Second pass: Fill remaining slots with Unicode labels
        for glyph_def in &yaff_font.glyphs {
            for label in &glyph_def.labels {
                if let Label::Unicode(codes) = label {
                    for &code in codes {
                        if (code as usize) < 256 {
                            let target = &mut glyphs[code as usize];
                            // Only fill if empty (all zeros)
                            if target.data.iter().all(|&b| b == 0) {
                                for (y, row) in glyph_def.bitmap.pixels.iter().enumerate() {
                                    if y >= 32 {
                                        break;
                                    }
                                    let mut packed: u8 = 0;
                                    for (x, &pixel) in row.iter().enumerate() {
                                        if x >= 8 {
                                            break;
                                        }
                                        if pixel {
                                            packed |= 1 << (7 - x);
                                        }
                                    }
                                    target.data[y] = packed;
                                }
                            }
                        }
                    }
                }
            }
        }

        Self {
            name: name.into(),
            width,
            height,
            glyphs,
            path_opt: None,
            font_type: BitFontType::Custom,
        }
    }
}

impl FromStr for BitFont {
    type Err = EngineError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Try to load from file path or font name
        BitFont::from_sauce_name(s)
    }
}

// ========================================
// Legacy Constants for Backward Compatibility
// ========================================

/// Number of ANSI font slots (legacy alias)
pub const ANSI_FONTS: usize = 42;
