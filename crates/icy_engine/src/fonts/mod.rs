use base64::{Engine, engine::general_purpose};
#[allow(unused_imports)]
use lazy_static::lazy_static;
use libyaff::{GlyphDefinition, YaffFont};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{EngineError, FontError};
use parking_lot::Mutex;
use std::{collections::HashMap, path::PathBuf, str::FromStr, sync::Arc};

pub mod ansi;
pub mod legacy;
pub mod rip;
pub mod sauce;
pub mod skypix;

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

#[derive(Debug)]
pub struct BitFont {
    pub yaff_font: YaffFont,
    glyph_cache: Mutex<HashMap<char, GlyphDefinition>>,
    /// Pre-computed lookup table for ASCII/extended ASCII range (0..256)
    /// Codepoint labels have priority over Unicode labels
    glyph_lookup: Arc<[Option<GlyphDefinition>; 256]>,
    pub path_opt: Option<PathBuf>,
    font_type: BitFontType,
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
        let size = self.size();
        BitFontSerde {
            name: self.name().to_string(),
            width: size.width as u8,
            height: size.height as u8,
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
        self.yaff_font == other.yaff_font
    }
}

impl Clone for BitFont {
    fn clone(&self) -> Self {
        Self {
            yaff_font: self.yaff_font.clone(),
            glyph_cache: Mutex::new(self.glyph_cache.lock().clone()),
            glyph_lookup: self.glyph_lookup.clone(),
            path_opt: self.path_opt.clone(),
            font_type: self.font_type,
        }
    }
}

impl Default for BitFont {
    fn default() -> Self {
        BitFont::from_sauce_name("IBM VGA").unwrap()
    }
}

impl BitFont {
    pub fn name(&self) -> &str {
        self.yaff_font.name.as_deref().unwrap_or("")
    }

    pub fn set_name(&mut self, name: &str) {
        self.yaff_font.name = Some(name.to_string());
    }

    pub fn size(&self) -> Size {
        let mut width: i32 = 8;
        let mut height: i32 = 16;
        if let Some(h) = self.yaff_font.pixel_size {
            height = h;
        } else if let Some(h) = self.yaff_font.size {
            height = h;
        } else if let Some(h) = self.yaff_font.line_height {
            height = h;
        } else if let Some((_x, y)) = self.yaff_font.bounding_box {
            height = y as i32;
        } else if let Some((_x, y)) = self.yaff_font.cell_size {
            height = y as i32;
        }

        if let Some(cs) = self.yaff_font.bounding_box {
            width = cs.0 as i32;
        } else if let Some((x, _y)) = self.yaff_font.cell_size {
            width = x as i32;
        }

        Size::new(width as i32, height as i32)
    }

    pub fn font_type(&self) -> BitFontType {
        self.font_type
    }

    pub fn is_default(&self) -> bool {
        self.name() == ansi::DEFAULT_FONT_NAME || 
        self.name() == ansi::ALT_DEFAULT_FONT_NAME
    }

    /// Build a lookup table for chars 0..256
    /// Codepoint labels have priority over Unicode labels
    fn build_glyph_lookup(yaff_font: &YaffFont) -> [Option<GlyphDefinition>; 256] {
        use libyaff::Label;

        let mut lookup: [Option<GlyphDefinition>; 256] = std::array::from_fn(|_| None);

        // First pass: Fill in glyphs with codepoint labels (highest priority)
        for glyph in &yaff_font.glyphs {
            for label in &glyph.labels {
                if let Label::Codepoint(codes) = label {
                    for &code in codes {
                        if (code as usize) < 256 && lookup[code as usize].is_none() {
                            lookup[code as usize] = Some(glyph.clone());
                        }
                    }
                }
            }
        }

        // Second pass: Fill remaining slots with Unicode labels
        for glyph in &yaff_font.glyphs {
            for label in &glyph.labels {
                if let Label::Unicode(codes) = label {
                    for &code in codes {
                        if (code as usize) < 256 && lookup[code as usize].is_none() {
                            lookup[code as usize] = Some(glyph.clone());
                        }
                    }
                }
            }
        }

        lookup
    }

    /// Get a glyph for the given character, using lookup table for ASCII range
    pub fn glyph(&self, ch: char) -> Option<GlyphDefinition> {
        let ch_code = ch as u32;

        // Use pre-built lookup table for ASCII/extended ASCII range (0..256)
        if ch_code < 256 {
            return self.glyph_lookup[ch_code as usize].clone();
        }

        // For characters outside 0..256, check cache first
        {
            let cache = self.glyph_cache.lock();
            if let Some(glyph_def) = cache.get(&ch) {
                return Some(glyph_def.clone());
            }
        }

        // Find and cache the glyph
        if let Some(glyph_def) = self.find_glyph_in_font(ch) {
            self.glyph_cache.lock().insert(ch, glyph_def.clone());
            return Some(glyph_def);
        }

        None
    }

    /// Find a glyph definition for the given character
    fn find_glyph_in_font(&self, ch: char) -> Option<GlyphDefinition> {
        use libyaff::Label;

        let result = self
            .yaff_font
            .glyphs
            .iter()
            .find(|g: &&GlyphDefinition| {
                let matches = g.labels.iter().any(|label| match label {
                    Label::Codepoint(codes) => {
                        let match_found = codes.contains(&(ch as u16));
                        match_found
                    }
                    Label::Unicode(codes) => {
                        let match_found = codes.contains(&(ch as u32));
                        match_found
                    }
                    _ => false,
                });
                matches
            })
            .cloned();

        result
    }

    /// Convert font to raw u8 data for legacy formats
    pub fn convert_to_u8_data(&self) -> Vec<u8> {
        let mut result = Vec::new();
        let size = self.size();
        let length = 256; // Standard ASCII range

        for ch_code in 0..length {
            let ch = unsafe { char::from_u32_unchecked(ch_code as u32) };
            if let Some(glyph_def) = self.find_glyph_in_font(ch) {
                // Convert bitmap to u8 rows
                let mut rows = Vec::new();
                let height = glyph_def.bitmap.height;
                let width = glyph_def.bitmap.width;

                for y in 0..height {
                    let mut packed: u8 = 0;
                    if y < glyph_def.bitmap.pixels.len() {
                        let row = &glyph_def.bitmap.pixels[y];
                        for x in 0..width.min(8) {
                            if x < row.len() && row[x] {
                                packed |= 1 << (7 - x);
                            }
                        }
                    }
                    rows.push(packed);
                }

                // Normalize to font height
                let target = size.height as usize;
                if rows.len() > target {
                    rows.truncate(target);
                } else if rows.len() < target {
                    rows.resize(target, 0);
                }
                result.extend_from_slice(&rows);
            } else {
                // No glyph found, add empty rows
                result.extend_from_slice(vec![0; size.height as usize].as_slice());
            }
        }
        result
    }

    pub fn encode_as_ansi(&self, font_slot: usize) -> String {
        let font_data = self.convert_to_u8_data();
        let data = general_purpose::STANDARD.encode(font_data);
        format!("\x1BPCTerm:Font:{font_slot}:{data}\x1B\\")
    }

    /// Create a font from raw 8-bit data
    pub fn create_8(name: impl Into<String>, width: u8, height: u8, data: &[u8]) -> Self {
        let mut yaff_font = YaffFont::from_raw_bytes(data, width as u32, height as u32).unwrap();
        yaff_font.name = Some(name.into());
        let glyph_lookup = Self::build_glyph_lookup(&yaff_font);
        Self {
            path_opt: None,
            font_type: BitFontType::Custom,
            yaff_font,
            glyph_cache: Mutex::new(HashMap::new()),
            glyph_lookup: Arc::new(glyph_lookup),
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
        // Use libyaff to convert to PSF2 format
        Ok(libyaff::psf::to_psf2_bytes(&self.yaff_font)?)
    }

    /// Clone this font with a different line height using Atari-style row replication.
    /// This uses a Bresenham-like error accumulator so small size changes (e.g. 8->9)
    /// keep consistent baseline & stroke thickness, closer to original VDI behaviour.
    pub fn scale_to_height(&self, new_height: i32) -> crate::Result<Self> {
        use libyaff::Bitmap;
        if new_height == self.size().height {
            return Ok(self.clone());
        }

        let old_height = self.size().height.max(1) as usize;
        let target_height = new_height.max(1) as usize;

        let mut yaff_font = self.yaff_font.clone();
        yaff_font.line_height = Some(target_height as i32);
        yaff_font.bounding_box = Some((self.size().width as u32, target_height as u32));
        yaff_font.cell_size = Some((self.size().width as u32, target_height as u32));

        for glyph in yaff_font.glyphs.iter_mut() {
            let old_pixels = &glyph.bitmap.pixels;
            let mut new_pixels: Vec<Vec<bool>> = Vec::with_capacity(target_height);
            let mut err: isize = 0;
            let mut src_row: usize = 0;
            while new_pixels.len() < target_height {
                // Vertical replication
                let src_vec = old_pixels.get(src_row).cloned().unwrap_or_else(|| vec![false; glyph.bitmap.width as usize]);
                // Keep original width; Atari ST VDI did not scale width proportionally for simple height changes
                new_pixels.push(src_vec);
                err += old_height as isize;
                if err >= target_height as isize {
                    err -= target_height as isize;
                    src_row = (src_row + 1).min(old_pixels.len().saturating_sub(1));
                }
            }

            let new_w = glyph.bitmap.width.max(1) as usize;
            glyph.bitmap = Bitmap {
                width: new_w,
                height: target_height,
                pixels: new_pixels,
            };
        }

        let glyph_lookup = BitFont::build_glyph_lookup(&yaff_font);
        Ok(Self {
            yaff_font,
            glyph_cache: Mutex::new(HashMap::new()),
            glyph_lookup: Arc::new(glyph_lookup),
            path_opt: None,
            font_type: self.font_type,
        })
    }
}

impl BitFont {
    /// Load font from bytes (PSF1, PSF2, or plain format)
    pub fn from_bytes(name: impl Into<String>, data: &[u8]) -> crate::Result<Self> {
        // Try to parse as YaffFont first (handles PSF1, PSF2)
        match YaffFont::from_bytes(data) {
            Ok(mut yaff_font) => {
                yaff_font.name = Some(name.into());
                let glyph_lookup = Self::build_glyph_lookup(&yaff_font);
                Ok(Self {
                    path_opt: None,
                    font_type: BitFontType::BuiltIn,
                    yaff_font,
                    glyph_cache: Mutex::new(HashMap::new()),
                    glyph_lookup: Arc::new(glyph_lookup),
                })
            }
            Err(_) => {
                // Try as raw font data
                if data.len() % 256 != 0 {
                    return Err(FontError::UnknownFontFormat(data.len()).into());
                }
                let char_height = data.len() / 256;
                Ok(Self::create_8(name, 8, char_height as u8, data))
            }
        }
    }

    /// Double the size of this font by doubling each pixel point and line
    pub fn double_size(&self) -> Self {
        use libyaff::Bitmap;

        let old_size = self.size();
        let new_width = old_size.width * 2;
        let new_height = old_size.height * 2;

        let mut yaff_font = self.yaff_font.clone();
        yaff_font.line_height = Some(new_height);
        yaff_font.bounding_box = Some((new_width as u32, new_height as u32));
        yaff_font.cell_size = Some((new_width as u32, new_height as u32));

        for glyph in yaff_font.glyphs.iter_mut() {
            let old_pixels = &glyph.bitmap.pixels;
            let old_w = glyph.bitmap.width;
            let old_h = glyph.bitmap.height;

            let new_w = old_w * 2;
            let new_h = old_h * 2;

            let mut new_pixels: Vec<Vec<bool>> = Vec::with_capacity(new_h);

            // Double each row and each pixel in that row
            for row in old_pixels {
                let mut doubled_row = Vec::with_capacity(new_w);
                for &pixel in row {
                    // Each pixel becomes 2 pixels horizontally
                    doubled_row.push(pixel);
                    doubled_row.push(pixel);
                }
                // Each row appears twice vertically
                new_pixels.push(doubled_row.clone());
                new_pixels.push(doubled_row);
            }

            glyph.bitmap = Bitmap {
                width: new_w,
                height: new_h,
                pixels: new_pixels,
            };

            // Double the bearing and shift values as well (if present)
            glyph.left_bearing = glyph.left_bearing.map(|v| v * 2);
            glyph.right_bearing = glyph.right_bearing.map(|v| v * 2);
            glyph.shift_up = glyph.shift_up.map(|v| v * 2);
        }

        // Rebuild glyph lookup table for the new font
        let glyph_lookup = BitFont::build_glyph_lookup(&yaff_font);

        Self {
            yaff_font,
            glyph_cache: Mutex::new(HashMap::new()),
            glyph_lookup: Arc::new(glyph_lookup),
            path_opt: None,
            font_type: self.font_type,
        }
    }

    /// Convert this 8px wide font to a 9px wide font for VGA letter spacing mode.
    /// For box-drawing characters (CP437 0xC0-0xDF), the 8th pixel is extended to the 9th.
    /// For all other characters, the 9th pixel remains empty (background color).
    pub fn to_9px_font(&self) -> Self {
        use libyaff::Bitmap;

        let old_size = self.size();

        // Only convert 8px wide fonts
        if old_size.width != 8 {
            return self.clone();
        }

        // Build a new glyph lookup table with 9px wide glyphs
        // The key insight: glyph_lookup[i] contains the glyph for CP437 codepoint i
        // So we can directly check if i is in the range 0xC0-0xDF
        let mut new_glyph_lookup: [Option<libyaff::GlyphDefinition>; 256] = std::array::from_fn(|_| None);

        for cp437_code in 0..256 {
            if let Some(glyph) = &self.glyph_lookup[cp437_code] {
                let old_pixels = &glyph.bitmap.pixels;
                let old_h = glyph.bitmap.height;

                // Check if this is a box-drawing character (CP437 0xC0-0xDF)
                // These characters should have their 8th pixel extended to the 9th
                // This matches the original logic from the OpenGL renderer
                let extend_to_9th = (0xC0..=0xDF).contains(&cp437_code);

                let new_pixels: Vec<Vec<bool>> = old_pixels
                    .iter()
                    .map(|row| {
                        // Create a 9-element row initialized to false
                        let mut new_row = vec![false; 9];
                        // Copy the original pixels (up to 8)
                        let copy_len = row.len().min(8);
                        new_row[..copy_len].copy_from_slice(&row[..copy_len]);
                        // Add the 9th pixel: extend from 8th for box-drawing characters
                        // Only if the 8th pixel (index 7) is set
                        if extend_to_9th && row.len() >= 8 && row[7] {
                            new_row[8] = true;
                        }
                        new_row
                    })
                    .collect();

                let mut new_glyph = glyph.clone();
                new_glyph.bitmap = Bitmap {
                    width: 9,
                    height: old_h,
                    pixels: new_pixels,
                };
                new_glyph_lookup[cp437_code] = Some(new_glyph);
            }
        }

        // Also update the yaff_font for consistency (though we mainly use glyph_lookup)
        let mut yaff_font = self.yaff_font.clone();
        yaff_font.bounding_box = Some((9, old_size.height as u32));
        yaff_font.cell_size = Some((9, old_size.height as u32));

        Self {
            yaff_font,
            glyph_cache: Mutex::new(HashMap::new()),
            glyph_lookup: Arc::new(new_glyph_lookup),
            path_opt: None,
            font_type: self.font_type,
        }
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
