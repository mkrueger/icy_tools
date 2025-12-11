//! TheDraw Font (TDF) utilities
//!
//! This module provides utilities for working with retrofont's TdfFont,
//! using the retrofont types directly instead of custom glyph types.

use std::path::Path;

pub use retrofont::tdf::TdfFont;

use super::FontType;

/// Extension trait for TdfFont providing additional functionality
pub trait TdfFontExt {
    /// Load TDF fonts from a file
    fn load_from_file(path: &Path) -> anyhow::Result<Vec<TdfFont>>;

    /// Get the font type as our enum
    fn get_font_type(&self) -> FontType;

    /// Get spacing value with default
    fn get_spacing(&self) -> i32;
}

impl TdfFontExt for TdfFont {
    fn load_from_file(path: &Path) -> anyhow::Result<Vec<TdfFont>> {
        let data = std::fs::read(path)?;
        let fonts = TdfFont::load(&data)?;
        Ok(fonts)
    }

    fn get_font_type(&self) -> FontType {
        self.font_type.into()
    }

    fn get_spacing(&self) -> i32 {
        self.spacing
    }
}

/// Load TDF fonts from bytes
pub fn load_tdf_fonts(data: &[u8]) -> anyhow::Result<Vec<TdfFont>> {
    let fonts = TdfFont::load(data)?;
    if fonts.is_empty() {
        use retrofont::tdf::TdfFontType;
        // Return a default empty font if file had no fonts
        Ok(vec![TdfFont::new("New Font", TdfFontType::Color, 1)])
    } else {
        Ok(fonts)
    }
}
