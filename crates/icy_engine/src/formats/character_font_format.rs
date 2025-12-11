//! Character font file format definitions.
//!
//! This module defines the supported character/ASCII art font formats for loading.
//! These are "big letter" fonts used to render text as ASCII/ANSI art.

use std::path::Path;

/// Supported character font file formats.
///
/// Character fonts are ASCII/ANSI art fonts that render text as large
/// decorative characters made up of multiple smaller characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharacterFontFormat {
    /// FIGlet font format
    /// Extension: .flf
    Figlet,

    /// TheDraw font format
    /// Extension: .tdf
    Tdf,
}

impl CharacterFontFormat {
    /// Get the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Figlet => "flf",
            Self::Tdf => "tdf",
        }
    }

    /// Try to detect the format from a file extension.
    ///
    /// Accepts extensions with or without leading dot.
    ///
    /// # Examples
    /// ```
    /// use icy_engine::formats::CharacterFontFormat;
    ///
    /// assert_eq!(CharacterFontFormat::from_extension("flf"), Some(CharacterFontFormat::Figlet));
    /// assert_eq!(CharacterFontFormat::from_extension(".tdf"), Some(CharacterFontFormat::Tdf));
    /// assert_eq!(CharacterFontFormat::from_extension("TDF"), Some(CharacterFontFormat::Tdf));
    /// ```
    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.trim_start_matches('.').to_lowercase();

        match ext.as_str() {
            "flf" => Some(Self::Figlet),
            "tdf" => Some(Self::Tdf),
            _ => None,
        }
    }

    /// Try to detect the format from a file path.
    ///
    /// # Examples
    /// ```
    /// use icy_engine::formats::CharacterFontFormat;
    /// use std::path::Path;
    ///
    /// assert_eq!(CharacterFontFormat::from_path(Path::new("banner.flf")), Some(CharacterFontFormat::Figlet));
    /// assert_eq!(CharacterFontFormat::from_path(Path::new("cool.tdf")), Some(CharacterFontFormat::Tdf));
    /// ```
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension().and_then(|ext| ext.to_str()).and_then(Self::from_extension)
    }

    /// Check if a file extension matches a character font format.
    ///
    /// # Examples
    /// ```
    /// use icy_engine::formats::CharacterFontFormat;
    ///
    /// assert!(CharacterFontFormat::is_font_extension("flf"));
    /// assert!(CharacterFontFormat::is_font_extension("TDF"));
    /// assert!(!CharacterFontFormat::is_font_extension("txt"));
    /// ```
    pub fn is_font_extension(ext: &str) -> bool {
        Self::from_extension(ext).is_some()
    }

    /// Get the human-readable name of this format.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Figlet => "FIGlet Font",
            Self::Tdf => "TheDraw Font",
        }
    }

    /// Get a description of this format.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Figlet => "FIGlet ASCII art font format",
            Self::Tdf => "TheDraw ANSI art font format",
        }
    }
}
