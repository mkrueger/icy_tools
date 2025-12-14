//! Unified error types for icy_engine

use std::path::PathBuf;
use thiserror::Error;

/// Main error type for icy_engine operations
#[derive(Debug, Error)]
pub enum EngineError {
    // === I/O Errors ===
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to open file '{path}': {message}")]
    OpenFile { path: PathBuf, message: String },

    #[error("Failed to read file '{path}': {message}")]
    ReadFile { path: PathBuf, message: String },

    // === Loading Errors ===
    #[error("File too short to be valid")]
    FileTooShort,

    #[error("File length must be even")]
    FileLengthMustBeEven,

    #[error("Invalid file ID or magic number mismatch")]
    IdMismatch,

    #[error("Data out of bounds at offset {offset}")]
    OutOfBounds { offset: usize },

    #[error("Unsupported IcyDraw layer mode: {mode}")]
    UnsupportedLayerMode { mode: u8 },

    #[error("Unsupported ADF version: {version}")]
    UnsupportedAdfVersion { version: u8 },

    #[error("Invalid PNG data: {message}")]
    InvalidPng { message: String },

    #[error("Unsupported format: {description}")]
    UnsupportedFormat { description: String },

    // === Saving Errors ===
    #[error("No font found in buffer")]
    NoFontFound,

    #[error("Only 8x16 fonts are supported by this format")]
    Only8x16FontsSupported,

    #[error("Font not supported by XBin format (requires 8px width, 1-32px height)")]
    InvalidXBinFont,

    #[error("Only 8-bit characters are supported by this format")]
    Only8BitCharactersSupported,

    // === Font Errors ===
    #[error("Font not found")]
    FontNotFound,

    #[error("Invalid PSF file: magic number mismatch")]
    PsfMagicMismatch,

    #[error("Unsupported PSF version: {version}")]
    UnsupportedPsfVersion { version: u32 },

    #[error("Font data length mismatch: expected {expected}, got {actual}")]
    FontLengthMismatch { expected: usize, actual: usize },

    #[error("Unknown font format ({size} bytes). Valid heights: 8, 14, 16, 19")]
    UnknownFontFormat { size: usize },

    #[error("Unsupported font page: {page}")]
    UnsupportedFontPage { page: usize },

    #[error("Unsupported SAUCE font: {name}")]
    UnsupportedSauceFont { name: String },

    // === Palette Errors ===
    #[error("Invalid hex color: {value}")]
    InvalidHexColor { value: String },

    #[error("Invalid palette format: {message}")]
    InvalidPaletteFormat { message: String },

    #[error("Unsupported palette format: expected {expected}")]
    UnsupportedPaletteFormat { expected: String },

    // === Buffer Errors ===
    #[error("Buffer type mismatch: expected {expected}")]
    BufferTypeMismatch { expected: String },

    #[error("Not implemented: {feature}")]
    NotImplemented { feature: String },

    #[error("Layer {layer} out of range (0..{max})")]
    LayerOutOfRange { layer: usize, max: usize },

    // === Format Restrictions ===
    #[error("Only 16 color palettes are supported by this format")]
    Only16ColorPalettesSupported,

    #[error("Only ice mode files are supported by this format")]
    OnlyIceModeSupported,

    #[error("Only width=={width} files are supported by this format")]
    WidthNotSupported { width: usize },

    #[error("Only up to {max_lines} lines are supported by this format")]
    TooManyLines { max_lines: usize },

    #[error("Only single font files are supported by this format")]
    OnlySingleFontSupported,

    #[error("Format '{}' does not support {operation}", name)]
    FormatNotSupported { name: String, operation: String },

    #[error("Invalid bounds: {message}")]
    InvalidBounds { message: String },

    // === XBin Format Errors ===
    #[error("Invalid XBin: {message}")]
    InvalidXBin { message: String },

    // === Sixel Errors ===
    #[error("Sixel decode error: {message}")]
    SixelDecodeError { message: String },

    // === GIF Errors ===
    #[error("GIF writer thread panicked")]
    GifWriterPanicked,

    #[error("Failed to create image buffer")]
    ImageBufferCreationFailed,

    #[error("Failed to save image: {message}")]
    ImageSaveFailed { message: String },

    // === External Errors ===
    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("PNG encoding error: {0}")]
    PngEncoding(#[from] png::EncodingError),

    #[error("PNG decoding error: {0}")]
    PngDecoding(#[from] png::DecodingError),

    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("Parse int error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    // === GIF Encoder Errors ===
    #[error("GIF encoder error: {0}")]
    GifEncoder(#[from] gif::EncodingError),

    // === SAUCE Errors ===
    #[error("SAUCE error: {0}")]
    Sauce(#[from] icy_sauce::SauceError),

    // === Libyaff Errors ===
    #[error("Font parsing error: {0}")]
    FontParse(#[from] libyaff::ParseError),

    #[error("{0}")]
    Generic(String),

    #[error("Archive error: {0}")]
    Archive(#[from] unarc_rs::error::ArchiveError),
}

/// Result type alias for icy_engine operations
pub type Result<T> = std::result::Result<T, EngineError>;

// === Convenience constructors ===
impl EngineError {
    /// Create an error for unsupported features
    pub fn not_implemented(feature: impl Into<String>) -> Self {
        Self::NotImplemented { feature: feature.into() }
    }

    /// Create a generic error from any displayable type
    pub fn generic(msg: impl std::fmt::Display) -> Self {
        Self::Generic(msg.to_string())
    }

    /// Create an open file error
    pub fn open_file(path: impl Into<PathBuf>, msg: impl Into<String>) -> Self {
        Self::OpenFile {
            path: path.into(),
            message: msg.into(),
        }
    }

    /// Create a read file error
    pub fn read_file(path: impl Into<PathBuf>, msg: impl Into<String>) -> Self {
        Self::ReadFile {
            path: path.into(),
            message: msg.into(),
        }
    }
}

// === Backward compatibility: From impls for legacy error types ===

/// Legacy LoadingError variants - will be removed in future
#[derive(Debug, Clone)]
pub enum LoadingError {
    OpenFileError(String),
    Error(String),
    ReadFileError(String),
    FileTooShort,
    IcyDrawUnsupportedLayerMode(u8),
    InvalidPng(String),
    UnsupportedADFVersion(u8),
    FileLengthNeedsToBeEven,
    IDMismatch,
    OutOfBounds,
}

impl From<LoadingError> for EngineError {
    fn from(err: LoadingError) -> Self {
        match err {
            LoadingError::OpenFileError(msg) => EngineError::Generic(format!("Error opening file: {msg}")),
            LoadingError::Error(msg) => EngineError::Generic(msg),
            LoadingError::ReadFileError(msg) => EngineError::Generic(format!("Error reading file: {msg}")),
            LoadingError::FileTooShort => EngineError::FileTooShort,
            LoadingError::IcyDrawUnsupportedLayerMode(mode) => EngineError::UnsupportedLayerMode { mode },
            LoadingError::InvalidPng(msg) => EngineError::InvalidPng { message: msg },
            LoadingError::UnsupportedADFVersion(version) => EngineError::UnsupportedAdfVersion { version },
            LoadingError::FileLengthNeedsToBeEven => EngineError::FileLengthMustBeEven,
            LoadingError::IDMismatch => EngineError::IdMismatch,
            LoadingError::OutOfBounds => EngineError::OutOfBounds { offset: 0 },
        }
    }
}

/// Legacy SavingError variants - will be removed in future
#[derive(Debug, Clone)]
pub enum SavingError {
    NoFontFound,
    Only8x16FontsSupported,
    InvalidXBinFont,
    Only8BitCharactersSupported,
}

impl From<SavingError> for EngineError {
    fn from(err: SavingError) -> Self {
        match err {
            SavingError::NoFontFound => EngineError::NoFontFound,
            SavingError::Only8x16FontsSupported => EngineError::Only8x16FontsSupported,
            SavingError::InvalidXBinFont => EngineError::InvalidXBinFont,
            SavingError::Only8BitCharactersSupported => EngineError::Only8BitCharactersSupported,
        }
    }
}

/// Legacy FontError variants - will be removed in future
#[derive(Debug, Clone)]
pub enum FontError {
    FontNotFound,
    MagicNumberMismatch,
    UnsupportedVersion(u32),
    LengthMismatch(usize, usize),
    UnknownFontFormat(usize),
}

impl From<FontError> for EngineError {
    fn from(err: FontError) -> Self {
        match err {
            FontError::FontNotFound => EngineError::FontNotFound,
            FontError::MagicNumberMismatch => EngineError::PsfMagicMismatch,
            FontError::UnsupportedVersion(v) => EngineError::UnsupportedPsfVersion { version: v },
            FontError::LengthMismatch(actual, expected) => EngineError::FontLengthMismatch { expected, actual },
            FontError::UnknownFontFormat(size) => EngineError::UnknownFontFormat { size },
        }
    }
}

/// Legacy ParserError variants - will be removed in future
#[derive(Debug, Clone)]
pub enum ParserError {
    UnsupportedFont(usize),
    UnsupportedSauceFont(String),
}

impl From<ParserError> for EngineError {
    fn from(err: ParserError) -> Self {
        match err {
            ParserError::UnsupportedFont(page) => EngineError::UnsupportedFontPage { page },
            ParserError::UnsupportedSauceFont(name) => EngineError::UnsupportedSauceFont { name },
        }
    }
}
