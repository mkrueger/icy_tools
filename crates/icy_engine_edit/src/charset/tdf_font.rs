//! TheDraw Font (TDF) utilities
//!
//! This module provides utilities for working with retrofont's TdfFont,
//! using the retrofont types directly instead of custom glyph types.

use std::path::Path;

pub use retrofont::tdf::TdfFont;

/// Load TDF fonts from a file path
pub fn load_tdf_fonts_from_file(path: &Path) -> anyhow::Result<Vec<TdfFont>> {
    let data = std::fs::read(path)?;
    load_tdf_fonts(&data)
}

/// Load TDF fonts from bytes
pub fn load_tdf_fonts(data: &[u8]) -> anyhow::Result<Vec<TdfFont>> {
    use retrofont::tdf::TdfFontType;
    let fonts = TdfFont::load(data)?;
    if fonts.is_empty() {
        // Return a default empty font if file had no fonts
        Ok(vec![TdfFont::new("New Font", TdfFontType::Color, 1)])
    } else {
        Ok(fonts)
    }
}
