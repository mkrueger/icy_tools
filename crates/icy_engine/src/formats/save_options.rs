//! Unified save options for all file formats.
//!
//! This module provides a clean, hierarchical save options structure.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::ScreenPreperation;
pub use icy_sauce::MetaData as SauceMetaData;

/// Main save options structure for all file formats.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SaveOptions {
    /// Optional SAUCE metadata to append.
    /// The actual `SauceRecord` will be created by the backend with appropriate capabilities.
    #[serde(skip)]
    pub sauce: Option<SauceMetaData>,

    /// Preprocessing options applied before saving.
    #[serde(default)]
    pub preprocess: PreprocessOptions,

    /// Format-specific options.
    #[serde(default)]
    pub format: FormatOptions,
}

impl fmt::Debug for SaveOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SaveOptions")
            .field("sauce", &self.sauce.as_ref().map(|_| "SauceMetaData"))
            .field("preprocess", &self.preprocess)
            .field("format", &self.format)
            .finish()
    }
}

impl SaveOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create save options with ANSI format settings.
    pub fn ansi(level: AnsiCompatibilityLevel) -> Self {
        Self {
            format: FormatOptions::Ansi(AnsiFormatOptions::new(level)),
            ..Default::default()
        }
    }

    /// Create save options for `IcyDraw` native format.
    pub fn icy_draw() -> Self {
        Self {
            format: FormatOptions::IcyDraw(IcyDrawFormatOptions::default()),
            ..Default::default()
        }
    }

    /// Get the ANSI format options, or default if not set.
    pub fn ansi_options(&self) -> AnsiFormatOptions {
        match &self.format {
            FormatOptions::Ansi(opts) => opts.clone(),
            _ => AnsiFormatOptions::default(),
        }
    }

    /// Get the `IcyDraw` format options, or default if not set.
    pub fn icy_draw_options(&self) -> IcyDrawFormatOptions {
        match &self.format {
            FormatOptions::IcyDraw(opts) => opts.clone(),
            _ => IcyDrawFormatOptions::default(),
        }
    }

    /// Get the compressed format options, or default if not set.
    pub fn compressed_options(&self) -> CompressedFormatOptions {
        match &self.format {
            FormatOptions::Compressed(opts) => opts.clone(),
            _ => CompressedFormatOptions::default(),
        }
    }

    /// Get the character format options, or default if not set.
    pub fn character_options(&self) -> CharacterFormatOptions {
        match &self.format {
            FormatOptions::Character(opts) => opts.clone(),
            _ => CharacterFormatOptions::default(),
        }
    }

    /// Returns true if lossless output is requested (no color optimization).
    pub fn is_lossless(&self) -> bool {
        !self.preprocess.optimize_colors
    }
}

/// Preprocessing options applied to buffer before format-specific saving.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreprocessOptions {
    /// When true, optimize colors (opposite of old `lossless_output`).
    /// Ignores fg color changes in whitespaces and bg color changes in blocks.
    pub optimize_colors: bool,

    /// When true, all whitespace characters will be normalized to spaces.
    pub normalize_whitespaces: bool,
}

impl Default for PreprocessOptions {
    fn default() -> Self {
        Self {
            optimize_colors: true,
            normalize_whitespaces: true,
        }
    }
}

/// Format-specific options.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum FormatOptions {
    /// No format-specific options needed.
    #[default]
    None,

    /// ANSI terminal format options.
    Ansi(AnsiFormatOptions),

    /// Character-based formats (ASCII, `PCBoard`, Avatar, `CtrlA`, Renegade).
    Character(CharacterFormatOptions),

    /// Formats that support compression (`XBin`).
    Compressed(CompressedFormatOptions),

    /// `IcyDraw` native format options.
    IcyDraw(IcyDrawFormatOptions),
}

/// Options for character-based formats (ASCII, `PCBoard`, Avatar, `CtrlA`, Renegade).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CharacterFormatOptions {
    /// Screen preparation sequence.
    pub screen_prep: ScreenPreperation,

    /// Output as Unicode (UTF-8) instead of native charset.
    pub unicode: bool,
}

/// Options for formats supporting compression (`XBin`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CompressedFormatOptions {
    /// Enable RLE compression.
    pub compress: bool,
}

/// Options for `IcyDraw` native format.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IcyDrawFormatOptions {
    /// Skip generating the thumbnail image (faster for autosave).
    pub skip_thumbnail: bool,

    /// Enable compression.
    pub compress: bool,
}

impl Default for IcyDrawFormatOptions {
    fn default() -> Self {
        Self {
            skip_thumbnail: false,
            compress: true,
        }
    }
}

/// ANSI compatibility level for output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AnsiCompatibilityLevel {
    /// Strictest output targeting DOS `ANSI.SYS`.
    AnsiSys,

    /// DEC VT100-ish baseline (still 7-bit/8-bit text, 16 colors).
    #[default]
    Vt100,

    /// IcyTerm/SyncTerm class terminals (256 colors / truecolor / REP / sixel).
    IcyTerm,

    /// Modern UTF-8 terminal (truecolor / UTF-8 output / sixel).
    Utf8Terminal,
}

impl std::fmt::Display for AnsiCompatibilityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnsiCompatibilityLevel::AnsiSys => write!(f, "ANSI.SYS"),
            AnsiCompatibilityLevel::Vt100 => write!(f, "VT-100"),
            AnsiCompatibilityLevel::IcyTerm => write!(f, "IcyTerm"),
            AnsiCompatibilityLevel::Utf8Terminal => write!(f, "UTF-8"),
        }
    }
}

impl AnsiCompatibilityLevel {
    /// Returns all variants for use in pick lists.
    pub fn all() -> &'static [AnsiCompatibilityLevel] {
        &[
            AnsiCompatibilityLevel::AnsiSys,
            AnsiCompatibilityLevel::Vt100,
            AnsiCompatibilityLevel::IcyTerm,
            AnsiCompatibilityLevel::Utf8Terminal,
        ]
    }

    /// Returns true if this level supports UTF-8 output.
    pub fn supports_utf8(self) -> bool {
        matches!(self, Self::Utf8Terminal)
    }

    /// Returns true if this level supports 256-color mode.
    pub fn supports_256_colors(self) -> bool {
        matches!(self, Self::IcyTerm | Self::Utf8Terminal)
    }

    /// Returns true if this level supports 24-bit truecolor.
    pub fn supports_truecolor(self) -> bool {
        matches!(self, Self::IcyTerm | Self::Utf8Terminal)
    }

    /// Returns true if this level supports SIXEL graphics.
    pub fn supports_sixel(self) -> bool {
        matches!(self, Self::IcyTerm | Self::Utf8Terminal)
    }

    /// Returns true if this level supports cursor forward (CUF) sequences.
    pub fn supports_cursor_forward(self) -> bool {
        matches!(self, Self::Vt100 | Self::IcyTerm | Self::Utf8Terminal)
    }

    /// Returns true if this level supports repeat (REP) sequences.
    pub fn supports_repeat(self) -> bool {
        matches!(self, Self::IcyTerm | Self::Utf8Terminal)
    }

    /// Returns true if this level supports cursor save/restore.
    pub fn supports_cursor_save_restore(self) -> bool {
        !matches!(self, Self::AnsiSys)
    }

    /// Returns true if this level supports font page switching.
    pub fn supports_font_pages(self) -> bool {
        matches!(self, Self::IcyTerm | Self::Utf8Terminal)
    }
}

/// Line length handling for ANSI output.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum LineLength {
    /// Use buffer width as line length.
    #[default]
    Default,

    /// Minimum line length (pad shorter lines).
    Minimum(u16),

    /// Maximum line length (wrap or truncate longer lines).
    Maximum(u16),
}

/// Line break behavior for ANSI output.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum LineBreakBehavior {
    /// Normal line wrapping.
    #[default]
    Wrap,

    /// Force line breaks at line end.
    Force,

    /// Use `GotoXY` sequences at line start (for longer terminals).
    GotoXY,
}

/// Line ending style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineEnding {
    /// Unix-style line feed only.
    #[default]
    Lf,

    /// DOS/Windows-style carriage return + line feed.
    CrLf,
}

/// Control character (0x00-0x1F) handling in output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlCharHandling {
    /// Output control characters directly.
    #[default]
    Ignore,

    /// Escape control characters using `IcyTerm` convention (ESC + char).
    IcyTerm,

    /// Replace control characters with a placeholder (e.g., '.').
    FilterOut,
}

/// Settings for SIXEL image encoding.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SixelSettings {
    /// Maximum number of colors in the palette (2-256).
    pub max_colors: u16,

    /// Floyd-Steinberg error diffusion strength (0.0-1.0).
    pub diffusion: f32,

    /// Use K-means clustering instead of Wu's quantizer.
    pub use_kmeans: bool,
}

impl Default for SixelSettings {
    fn default() -> Self {
        Self {
            max_colors: 256,
            diffusion: 0.875,
            use_kmeans: false,
        }
    }
}

impl SixelSettings {
    /// Convert to `icy_sixel::EncodeOptions`.
    pub fn to_encode_options(&self) -> icy_sixel::EncodeOptions {
        icy_sixel::EncodeOptions {
            max_colors: self.max_colors,
            diffusion: self.diffusion,
            quantize_method: if self.use_kmeans {
                icy_sixel::QuantizeMethod::kmeans()
            } else {
                icy_sixel::QuantizeMethod::Wu
            },
        }
    }
}

/// ANSI format-specific options.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnsiFormatOptions {
    /// Compatibility level determining available features.
    pub level: AnsiCompatibilityLevel,

    /// When true, always emit 24-bit truecolor (RGB) escape sequences for colors.
    /// This bakes the current palette colors into the output so the result looks
    /// the same even if the viewer's palette differs.
    #[serde(default)]
    pub always_use_rgb: bool,

    /// Screen preparation sequence.
    pub screen_prep: ScreenPreperation,

    /// Line length handling.
    pub line_length: LineLength,

    /// Line break behavior.
    pub line_break: LineBreakBehavior,

    /// Line ending style.
    pub line_ending: LineEnding,

    /// Control character handling.
    pub control_char_handling: ControlCharHandling,

    /// SIXEL encoding settings.
    pub sixel: SixelSettings,

    /// Lines to skip during output (runtime parameter for animation playback).
    /// This is not serialized - it's set programmatically by `icy_play`.
    #[serde(skip)]
    pub skip_lines: Vec<usize>,
}

impl Default for AnsiFormatOptions {
    fn default() -> Self {
        Self::new(AnsiCompatibilityLevel::default())
    }
}

impl AnsiFormatOptions {
    pub fn new(level: AnsiCompatibilityLevel) -> Self {
        Self {
            level,
            always_use_rgb: false,
            screen_prep: ScreenPreperation::None,
            line_length: LineLength::Default,
            line_break: LineBreakBehavior::Wrap,
            line_ending: LineEnding::Lf,
            control_char_handling: ControlCharHandling::Ignore,
            sixel: SixelSettings::default(),
            skip_lines: Vec::new(),
        }
    }

    /// Create options for modern terminal output.
    pub fn modern() -> Self {
        Self::new(AnsiCompatibilityLevel::Utf8Terminal)
    }

    /// Create options for IcyTerm/SyncTerm compatible output.
    pub fn icy_term() -> Self {
        Self::new(AnsiCompatibilityLevel::IcyTerm)
    }

    /// Create options for DOS ANSI.SYS compatible output.
    pub fn dos() -> Self {
        Self::new(AnsiCompatibilityLevel::AnsiSys)
    }
}

use bstr::BString;
use icy_sauce::{AspectRatio, BinaryCapabilities, Capabilities, CharacterCapabilities, CharacterFormat, LetterSpacing, SauceRecordBuilder};

use crate::{IceMode, TextBuffer, TextPane};

/// Trait to create SAUCE records with appropriate capabilities for different formats.
pub trait SauceBuilder {
    /// Create a `SauceRecord` with `CharacterCapabilities` (for ANSI, ASCII, Avatar, etc.)
    fn build_character_sauce(&self, meta: &SauceMetaData, format: CharacterFormat) -> icy_sauce::SauceRecord;

    /// Create a `SauceRecord` with `BinaryCapabilities` (for BIN, `XBin`)
    fn build_binary_sauce(&self, meta: &SauceMetaData) -> icy_sauce::SauceRecord;
}

impl SauceBuilder for TextBuffer {
    fn build_character_sauce(&self, meta: &SauceMetaData, format: CharacterFormat) -> icy_sauce::SauceRecord {
        let font_name = self.font_iter().next().map(|(_, font)| BString::from(super::guess_font_name(font)));

        let ice_colors = self.ice_mode == IceMode::Ice;

        let char_caps = CharacterCapabilities::with_font(
            format,
            self.width() as u16,
            self.height() as u16,
            ice_colors,
            LetterSpacing::EightPixel,
            AspectRatio::Square,
            font_name,
        )
        .unwrap_or_else(|_| CharacterCapabilities::new(format));

        // Use metadata() to set all fields at once, avoiding move issues
        let builder = SauceRecordBuilder::default()
            .metadata(meta.clone())
            .unwrap_or_else(|_| SauceRecordBuilder::default())
            .capabilities(Capabilities::Character(char_caps))
            .unwrap_or_else(|_| SauceRecordBuilder::default());

        builder.build()
    }

    fn build_binary_sauce(&self, meta: &SauceMetaData) -> icy_sauce::SauceRecord {
        let font_name = self.font_iter().next().map(|(_, font)| BString::from(super::guess_font_name(font)));

        let ice_colors = self.ice_mode == IceMode::Ice;

        // Create XBin capabilities (supports explicit width/height)
        let mut bin_caps = BinaryCapabilities::xbin(self.width().max(1) as u16, self.height().max(1) as u16).unwrap_or_else(|_| {
            // Fallback to minimal valid xbin
            BinaryCapabilities::xbin(1, 1).expect("1x1 should always be valid")
        });

        // Set additional properties
        bin_caps.ice_colors = ice_colors;
        bin_caps.letter_spacing = LetterSpacing::EightPixel;
        bin_caps.aspect_ratio = AspectRatio::Square;
        if let Some(font) = font_name {
            let _ = bin_caps.set_font(font);
        }

        // Use metadata() to set all fields at once, avoiding move issues
        let builder = SauceRecordBuilder::default()
            .metadata(meta.clone())
            .unwrap_or_else(|_| SauceRecordBuilder::default())
            .capabilities(Capabilities::Binary(bin_caps))
            .unwrap_or_else(|_| SauceRecordBuilder::default());

        builder.build()
    }
}
