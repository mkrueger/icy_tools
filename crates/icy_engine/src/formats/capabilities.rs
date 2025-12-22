//! Format capability system for compatibility checking.
//!
//! This module provides a system to check if a TextBuffer can be saved
//! in a specific file format without data loss.

use bitflags::bitflags;

bitflags! {
    /// Capabilities that a file format supports beyond its native character set.
    ///
    /// Every format can handle its native character set (CP437, PETSCII, etc.)
    /// and basic 16-color palette. These flags indicate additional capabilities.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct FormatCapabilities: u32 {
        /// Can store/display Unicode characters (beyond native charset like CP437)
        const UNICODE = 1 << 0;

        /// Can store 24-bit RGB colors (includes 256-color xterm as subset)
        const TRUECOLOR = 1 << 1;

        /// Can store a custom 16-color palette (not just default DOS/ANSI)
        const CUSTOM_PALETTE = 1 << 2;

        /// Supports iCE colors (high-intensity backgrounds instead of blink)
        const ICE_COLORS = 1 << 3;

        /// Can embed a custom font
        const CUSTOM_FONT = 1 << 4;

        /// XBin extended attributes (underline, strikethrough, etc. via font pages)
        const XBIN_EXTENDED = 1 << 5;

        /// Can store multiple/unlimited font slots
        const UNLIMITED_FONTS = 1 << 6;

        /// Supports SIXEL graphics
        const SIXEL = 1 << 7;

        /// Can represent control characters (0x00-0x1F) in output
        const CONTROL_CHARS = 1 << 8;

        /// Width must be even number (BIN format constraint - this is a REQUIREMENT not capability)
        const REQUIRE_EVEN_WIDTH = 1 << 9;
    }
}

/// Describes what features a buffer uses that need format support.
#[derive(Clone, Debug, Default)]
pub struct BufferCapabilityRequirements {
    /// The capabilities this buffer requires from a format
    pub required: FormatCapabilities,

    /// Buffer width (for format constraints)
    pub width: i32,

    /// Buffer height
    pub height: i32,

    /// Number of fonts used
    pub font_count: usize,

    /// Whether the palette differs from the default DOS palette
    pub has_custom_palette: bool,

    /// Whether any character uses RGB colors (not palette indices)
    pub uses_truecolor: bool,

    /// Whether ice colors are used (high-intensity backgrounds)
    pub uses_ice_colors: bool,

    /// Whether sixel graphics are present
    pub has_sixels: bool,

    /// Whether a non-default font is used
    pub has_custom_font: bool,

    /// Whether extended attributes (underline, strikethrough, etc.) are used
    pub uses_extended_attributes: bool,

    /// Whether control characters (0x00-0x1F) are present in the buffer
    pub has_control_chars: bool,
}

/// A compatibility issue found when checking format compatibility.
#[derive(Clone, Debug)]
pub struct CompatibilityIssue {
    /// Severity of the issue
    pub severity: IssueSeverity,

    /// Type of the issue
    pub issue_type: IssueType,

    /// Human-readable description
    pub message: String,
}

/// Severity of a compatibility issue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueSeverity {
    /// Data will be lost or corrupted - format cannot represent this feature
    Error,

    /// Some features won't be preserved exactly but file will work
    Warning,

    /// Minor change, format handles it differently
    Info,
}

/// Types of compatibility issues.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueType {
    /// Format doesn't support Unicode, will use native charset
    UnicodeUnsupported,

    /// Format doesn't support truecolor, will quantize to palette
    TruecolorUnsupported,

    /// Format doesn't support custom palettes
    CustomPaletteUnsupported,

    /// Format doesn't support ice colors
    IceColorsUnsupported,

    /// Format doesn't support custom fonts
    CustomFontUnsupported,

    /// Format doesn't support multiple fonts
    MultipleFontsUnsupported { font_count: usize },

    /// Format doesn't support sixel graphics
    SixelUnsupported,

    /// Format doesn't support extended attributes
    ExtendedAttributesUnsupported,

    /// Format doesn't support control characters
    ControlCharsUnsupported,

    /// Width must be even for this format
    OddWidthNotAllowed { width: i32 },

    /// Width exceeds format maximum
    WidthExceeded { width: i32, max: i32 },

    /// Height exceeds format maximum
    HeightExceeded { height: i32, max: i32 },
}

impl CompatibilityIssue {
    pub fn error(issue_type: IssueType, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Error,
            issue_type,
            message: message.into(),
        }
    }

    pub fn warning(issue_type: IssueType, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Warning,
            issue_type,
            message: message.into(),
        }
    }

    #[allow(dead_code)]
    pub fn info(issue_type: IssueType, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Info,
            issue_type,
            message: message.into(),
        }
    }
}
