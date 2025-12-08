//! BitFont file format definitions.
//!
//! This module defines the supported bitmap font formats for loading and saving.

use std::path::Path;

/// Supported bitmap font file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BitFontFormat {
    /// YAFF (Yet Another Font Format) - text-based bitmap font format
    /// Extension: .yaff
    Yaff,

    /// PSF (PC Screen Font) - Linux console font format
    /// Extension: .psf
    Psf,

    /// Raw bitmap font with specified height (8 pixels wide)
    /// Extension: .fXX where XX is the font height (e.g., .f08, .f14, .f16)
    Raw(u8),
}

impl BitFontFormat {
    /// Get the file extension for this format.
    ///
    /// For Raw format, returns a 2-digit formatted extension (e.g., "f08").
    pub fn extension(&self) -> String {
        match self {
            Self::Yaff => "yaff".to_string(),
            Self::Psf => "psf".to_string(),
            Self::Raw(height) => format!("f{:02}", height),
        }
    }

    /// Try to detect the format from a file extension.
    ///
    /// Accepts extensions with or without leading dot.
    /// For raw formats, accepts both single digit (.f8) and double digit (.f08) style.
    ///
    /// # Examples
    /// ```
    /// use icy_engine::formats::BitFontFormat;
    ///
    /// assert_eq!(BitFontFormat::from_extension("yaff"), Some(BitFontFormat::Yaff));
    /// assert_eq!(BitFontFormat::from_extension(".psf"), Some(BitFontFormat::Psf));
    /// assert_eq!(BitFontFormat::from_extension("f08"), Some(BitFontFormat::Raw(8)));
    /// assert_eq!(BitFontFormat::from_extension("f8"), Some(BitFontFormat::Raw(8)));
    /// assert_eq!(BitFontFormat::from_extension("f14"), Some(BitFontFormat::Raw(14)));
    /// ```
    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.trim_start_matches('.').to_lowercase();

        match ext.as_str() {
            "yaff" => Some(Self::Yaff),
            "psf" => Some(Self::Psf),
            _ if ext.starts_with('f') && ext.len() > 1 => {
                // Parse .fXX format (e.g., f08, f14, f16, f8)
                let height_str = &ext[1..];
                height_str.parse::<u8>().ok().map(Self::Raw)
            }
            _ => None,
        }
    }

    /// Try to detect the format from a file path.
    ///
    /// # Examples
    /// ```
    /// use icy_engine::formats::BitFontFormat;
    /// use std::path::Path;
    ///
    /// assert_eq!(BitFontFormat::from_path(Path::new("font.yaff")), Some(BitFontFormat::Yaff));
    /// assert_eq!(BitFontFormat::from_path(Path::new("console.psf")), Some(BitFontFormat::Psf));
    /// assert_eq!(BitFontFormat::from_path(Path::new("dos.f16")), Some(BitFontFormat::Raw(16)));
    /// ```
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension().and_then(|ext| ext.to_str()).and_then(Self::from_extension)
    }

    /// Check if a file extension matches a BitFont format.
    pub fn is_bitfont_extension(ext: &str) -> bool {
        Self::from_extension(ext).is_some()
    }

    /// Get all common BitFont extensions for file dialogs.
    pub fn all_extensions() -> &'static [&'static str] {
        &[
            "yaff", "psf", "f04", "f05", "f06", "f07", "f08", "f09", "f10", "f12", "f14", "f16", "f19", "f20", "f24", "f32",
        ]
    }

    /// Get a human-readable name for this format.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Yaff => "YAFF",
            Self::Psf => "PSF",
            Self::Raw(_) => "Raw Bitmap Font",
        }
    }

    /// Get a description of this format.
    pub fn description(&self) -> String {
        match self {
            Self::Yaff => "Yet Another Font Format (text-based)".to_string(),
            Self::Psf => "PC Screen Font (Linux console)".to_string(),
            Self::Raw(height) => format!("Raw bitmap font ({}px height)", height),
        }
    }
}

impl std::fmt::Display for BitFontFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Yaff => write!(f, "YAFF"),
            Self::Psf => write!(f, "PSF"),
            Self::Raw(height) => write!(f, "Raw ({}px)", height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_extension() {
        // YAFF
        assert_eq!(BitFontFormat::from_extension("yaff"), Some(BitFontFormat::Yaff));
        assert_eq!(BitFontFormat::from_extension("YAFF"), Some(BitFontFormat::Yaff));
        assert_eq!(BitFontFormat::from_extension(".yaff"), Some(BitFontFormat::Yaff));

        // PSF
        assert_eq!(BitFontFormat::from_extension("psf"), Some(BitFontFormat::Psf));
        assert_eq!(BitFontFormat::from_extension("PSF"), Some(BitFontFormat::Psf));

        // Raw formats
        assert_eq!(BitFontFormat::from_extension("f08"), Some(BitFontFormat::Raw(8)));
        assert_eq!(BitFontFormat::from_extension("f8"), Some(BitFontFormat::Raw(8)));
        assert_eq!(BitFontFormat::from_extension("f14"), Some(BitFontFormat::Raw(14)));
        assert_eq!(BitFontFormat::from_extension("f16"), Some(BitFontFormat::Raw(16)));
        assert_eq!(BitFontFormat::from_extension("F16"), Some(BitFontFormat::Raw(16)));
        assert_eq!(BitFontFormat::from_extension(".f19"), Some(BitFontFormat::Raw(19)));

        // Invalid
        assert_eq!(BitFontFormat::from_extension("txt"), None);
        assert_eq!(BitFontFormat::from_extension("f"), None);
        assert_eq!(BitFontFormat::from_extension(""), None);
    }

    #[test]
    fn test_from_path() {
        assert_eq!(BitFontFormat::from_path(Path::new("font.yaff")), Some(BitFontFormat::Yaff));
        assert_eq!(BitFontFormat::from_path(Path::new("/path/to/console.psf")), Some(BitFontFormat::Psf));
        assert_eq!(BitFontFormat::from_path(Path::new("dos.f16")), Some(BitFontFormat::Raw(16)));
        assert_eq!(BitFontFormat::from_path(Path::new("font.f08")), Some(BitFontFormat::Raw(8)));
        assert_eq!(BitFontFormat::from_path(Path::new("noext")), None);
    }

    #[test]
    fn test_extension() {
        assert_eq!(BitFontFormat::Yaff.extension(), "yaff");
        assert_eq!(BitFontFormat::Psf.extension(), "psf");
        assert_eq!(BitFontFormat::Raw(8).extension(), "f08");
        assert_eq!(BitFontFormat::Raw(14).extension(), "f14");
        assert_eq!(BitFontFormat::Raw(16).extension(), "f16");
    }

    #[test]
    fn test_is_bitfont_extension() {
        assert!(BitFontFormat::is_bitfont_extension("yaff"));
        assert!(BitFontFormat::is_bitfont_extension("psf"));
        assert!(BitFontFormat::is_bitfont_extension("f08"));
        assert!(BitFontFormat::is_bitfont_extension("f16"));
        assert!(!BitFontFormat::is_bitfont_extension("txt"));
        assert!(!BitFontFormat::is_bitfont_extension("ans"));
    }
}
