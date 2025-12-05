mod ansi;

use std::{error::Error, path::Path};

pub use ansi::*;

mod file_format;
pub use file_format::*;

mod image_format;
use icy_sauce::SauceRecord;
pub use image_format::*;

mod pcboard;
pub use pcboard::*;

mod avatar;
pub use avatar::*;

mod ascii;
pub use ascii::*;

mod bin;
pub use bin::*;

mod xbinary;
use serde::{Deserialize, Serialize};
pub use xbinary::*;

mod artworx;
pub use artworx::*;

mod ice_draw;
pub use ice_draw::*;

mod tundra;
pub use tundra::*;

mod color_optimization;
pub use color_optimization::*;
mod atascii;
mod ctrla;
mod icy_draw;
mod renegade;
mod seq;
pub use seq::seq_prepare;

use crate::{ANSI_FONTS, BitFont, BufferFeatures, EditableScreen, EngineResult, Layer, Role, SAUCE_FONT_NAMES, Screen, Size, TextPane, TextScreen};
use icy_parser_core::{CommandParser, MusicOption};

use super::{Position, TextAttribute};

#[derive(Default, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScreenPreperation {
    #[default]
    None,
    ClearScreen,
    Home,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ControlCharHandling {
    #[default]
    Ignore,
    IcyTerm,
    FilterOut,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SaveOptions {
    pub format_type: i32,

    pub screen_preparation: ScreenPreperation,
    pub modern_terminal_output: bool,

    #[serde(skip)]
    pub save_sauce: Option<SauceRecord>,

    /// When set the output will be compressed.
    pub compress: bool,

    /// When set the output will contain cursor forawad sequences. (CSI Ps C)
    pub use_cursor_forward: bool,
    /// When set the output will contain repeat sequences. (CSI Ps b)
    pub use_repeat_sequences: bool,

    /// When set the output will contain the full line length.
    /// This is useful for files that are meant to be displayed on a unix terminal where the bg color may not be 100% black.
    pub preserve_line_length: bool,

    /// When set the output will be cropped to this length.
    pub output_line_length: Option<usize>,

    /// When set the ansi engine will generate a gotoxy sequence at each line start
    ///  making the file work on longer terminals.
    pub longer_terminal_output: bool,

    /// When set output ignores fg color changes in whitespaces
    /// and bg color changes in blocks.
    pub lossles_output: bool,

    /// When set output will use extended color codes if they apply.
    pub use_extended_colors: bool,

    /// When set all whitespaces will be converted to spaces.
    pub normalize_whitespaces: bool,

    /// Changes control char output behavior
    pub control_char_handling: ControlCharHandling,

    #[serde(skip)]
    pub skip_lines: Option<Vec<usize>>,

    #[serde(skip)]
    pub alt_rgb: bool,

    #[serde(skip)]
    pub always_use_rgb: bool,
}

impl SaveOptions {
    pub const fn new() -> Self {
        SaveOptions {
            format_type: 0,
            longer_terminal_output: false,
            screen_preparation: ScreenPreperation::None,
            modern_terminal_output: false,
            save_sauce: None,
            compress: true,
            output_line_length: None,
            control_char_handling: ControlCharHandling::Ignore,
            lossles_output: false,
            use_extended_colors: true,
            normalize_whitespaces: true,
            use_cursor_forward: true,
            use_repeat_sequences: false,
            preserve_line_length: false,
            skip_lines: None,
            alt_rgb: false,
            always_use_rgb: false,
        }
    }
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct LoadData {
    sauce_opt: Option<icy_sauce::SauceRecord>,
    ansi_music: Option<MusicOption>,
    default_terminal_width: Option<usize>,
    /// Optional maximum height limit for the buffer
    /// If set, the buffer height will be clamped to this value after loading
    max_height: Option<i32>,
    convert_to_utf8: bool,
}

impl LoadData {
    pub fn new(sauce_opt: Option<icy_sauce::SauceRecord>, ansi_music: Option<MusicOption>, default_terminal_width: Option<usize>) -> Self {
        LoadData {
            sauce_opt,
            ansi_music,
            default_terminal_width,
            max_height: None,
            convert_to_utf8: false,
        }
    }

    pub fn convert_to_utf8(&self) -> bool {
        self.convert_to_utf8
    }

    pub fn with_convert_to_utf8(mut self, convert: bool) -> Self {
        self.convert_to_utf8 = convert;
        self
    }

    /// Set a maximum height limit for loading
    pub fn with_max_height(mut self, max_height: i32) -> Self {
        self.max_height = Some(max_height);
        self
    }

    /// Get the maximum height limit, if set
    pub fn max_height(&self) -> Option<i32> {
        self.max_height
    }
}

use crate::{IceMode, TextBuffer, limits};
use icy_sauce::prelude::*;

/// Apply SAUCE record settings directly to a TextBuffer.
/// This is used by formats that don't use TextScreen (like bin, xbinary, etc.)
pub fn apply_sauce_to_buffer(buf: &mut TextBuffer, sauce: &SauceRecord) {
    match sauce.capabilities() {
        Some(Capabilities::Character(CharacterCapabilities {
            columns,
            lines,
            font_opt,
            ice_colors,
            ..
        }))
        | Some(Capabilities::Binary(BinaryCapabilities {
            columns,
            lines,
            font_opt,
            ice_colors,
            ..
        })) => {
            // Apply buffer size (clamped to reasonable limits)
            if columns > 0 {
                let width = (columns as i32).min(limits::MAX_BUFFER_WIDTH);
                buf.set_width(width);
                buf.terminal_state.set_width(width);
            }

            if lines > 0 {
                let height = (lines as i32).min(limits::MAX_BUFFER_HEIGHT);
                buf.set_height(height);
            }

            // Resize first layer if needed
            if !buf.layers.is_empty() {
                let size = buf.get_size();
                buf.layers[0].set_size(size);
            }

            // Apply font if specified
            if let Some(font_name) = &font_opt {
                if let Ok(font) = BitFont::from_sauce_name(&font_name.to_string()) {
                    buf.set_font(0, font);
                }
            }

            // Apply ice colors
            if ice_colors {
                buf.ice_mode = IceMode::Ice;
            }
            buf.terminal_state.ice_colors = ice_colors;
        }
        _ => {
            // No character/binary capabilities - nothing to apply
        }
    }
}

pub trait OutputFormat: Send + Sync {
    fn get_file_extension(&self) -> &str;

    fn get_alt_extensions(&self) -> Vec<String> {
        Vec::new()
    }

    fn get_name(&self) -> &str;

    fn analyze_features(&self, _features: &BufferFeatures) -> String {
        String::new()
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn to_bytes(&self, buf: &mut crate::TextBuffer, options: &SaveOptions) -> anyhow::Result<Vec<u8>>;

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn load_buffer(&self, file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> anyhow::Result<crate::TextBuffer>;
}

lazy_static::lazy_static! {
    pub static ref FORMATS: [Box<dyn OutputFormat>; 14] = [
        Box::<ansi::Ansi>::default(),
        Box::<icy_draw::IcyDraw>::default(),
        Box::<IceDraw>::default(),
        Box::<Bin>::default(),
        Box::<XBin>::default(),
        Box::<TundraDraw>::default(),
        Box::<PCBoard>::default(),
        Box::<Avatar>::default(),
        Box::<ascii::Ascii>::default(),
        Box::<artworx::Artworx>::default(),
        Box::<ctrla::CtrlA>::default(),
        Box::<renegade::Renegade>::default(),
        Box::<seq::Seq>::default(),
        Box::<atascii::Atascii>::default(),
        ];
}

/// Parse data using a CommandParser from icy_parser_core
///
/// # Errors
///
/// Returns an error if sixel processing fails
pub fn load_with_parser(result: &mut TextScreen, interpreter: &mut dyn CommandParser, data: &[u8], _skip_errors: bool, min_height: i32) -> EngineResult<()> {
    use crate::ScreenSink;

    // Stop at EOF marker (Ctrl-Z)
    let data = if let Some(pos) = data.iter().position(|&b| b == 0x1A) {
        &data[..pos]
    } else {
        data
    };

    let mut sink = ScreenSink::new(result);
    interpreter.parse(data, &mut sink);

    // transform sixels to layers for non terminal buffers (makes sense in icy_draw for example)
    if !result.terminal_state().is_terminal_buffer {
        let mut num = 0;
        while !result.buffer.layers[0].sixels.is_empty() {
            if let Some(mut sixel) = result.buffer.layers[0].sixels.pop() {
                let size = sixel.get_size();
                let font_size = result.buffer.get_font_dimensions();
                let size = Size::new(
                    (size.width + font_size.width - 1) / font_size.width,
                    (size.height + font_size.height - 1) / font_size.height,
                );
                num += 1;
                let mut layer = Layer::new(format!("Sixel {}", num), size);
                layer.role = Role::Image;
                layer.set_offset(sixel.position);
                sixel.position = Position::default();
                layer.sixels.push(sixel);
                result.buffer.layers.push(layer);
            }
        }
    }

    // crop last empty line (if any)
    // get_line_count() returns the real height without empty lines
    // a caret move may move up, to load correctly it need to be checked.
    // The initial height of 24 lines may be too large for the real content height.
    if min_height > 0 {
        let real_height = result.buffer.get_line_count().max(result.caret.y + 1).max(min_height);
        result.buffer.set_height(real_height);
    }
    
    let height = result.get_height();
    let width = result.get_width();
    for y in 0..height {
        for x in 0..width {
            let mut ch = result.get_char((x, y).into());
            if ch.attribute.is_bold() {
                let fg = ch.attribute.get_foreground();
                if fg < 8 {
                    ch.attribute.set_foreground(fg + 8);
                }
                ch.attribute.set_is_bold(false);
                result.set_char((x, y).into(), ch);
            }
        }
    }
    Ok(())
}

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

impl std::fmt::Display for LoadingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadingError::Error(err) => write!(f, "Error while loading: {err}"),
            LoadingError::OpenFileError(err) => write!(f, "Error while opening file: {err}"),
            LoadingError::ReadFileError(err) => write!(f, "Error while reading file: {err}"),
            LoadingError::FileTooShort => write!(f, "File too short"),
            LoadingError::UnsupportedADFVersion(version) => {
                write!(f, "Unsupported ADF version: {version}")
            }
            LoadingError::IcyDrawUnsupportedLayerMode(mode) => {
                write!(f, "Unsupported layer mode: {mode}")
            }
            LoadingError::InvalidPng(err) => write!(f, "Error decoding PNG: {err}"),
            LoadingError::FileLengthNeedsToBeEven => write!(f, "File length needs to be even"),
            LoadingError::IDMismatch => write!(f, "ID mismatch"),
            LoadingError::OutOfBounds => write!(f, "Out of bounds"),
        }
    }
}

impl Error for LoadingError {
    fn description(&self) -> &str {
        "use std::display"
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

#[derive(Debug, Clone)]
pub enum SavingError {
    NoFontFound,
    Only8x16FontsSupported,
    InvalidXBinFont,
    Only8BitCharactersSupported,
}

impl std::fmt::Display for SavingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SavingError::NoFontFound => write!(f, "No font found"),
            SavingError::Only8x16FontsSupported => write!(f, "Only 8x16 fonts are supported by this format."),
            SavingError::InvalidXBinFont => write!(
                f,
                "font not supported by the .xb format only fonts with 8px width and a height from 1 to 32 are supported."
            ),
            SavingError::Only8BitCharactersSupported => write!(f, "Only 8 bit characters are supported by this format."),
        }
    }
}
impl Error for SavingError {
    fn description(&self) -> &str {
        "use std::display"
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

/// Prepare data for parsing.
/// Returns (data, is_unicode)
/// - If data has UTF-8 BOM: strip BOM and return (data, true)
/// - Otherwise: return data as-is with (data, false)
pub fn prepare_data_for_parsing(data: &[u8]) -> (&[u8], bool) {
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        // UTF-8 BOM detected - strip it and mark as unicode
        (&data[3..], true)
    } else {
        // No BOM - treat as raw bytes (CP437)
        (data, false)
    }
}

/// Legacy function - converts to String for backwards compatibility
/// Only use when you actually need a String
pub fn convert_ansi_to_utf8(data: &[u8]) -> (String, bool) {
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        if let Ok(result) = String::from_utf8(data[3..].to_vec()) {
            return (result, true);
        }
    }

    // interpret as raw bytes - each byte becomes a char
    // Note: This is only valid for bytes < 128, bytes >= 128 will become
    // unicode codepoints that don't match CP437!
    let mut result = String::new();
    for ch in data {
        let ch = *ch as char;
        result.push(ch);
    }
    (result, false)
}

pub fn guess_font_name(font: &BitFont) -> String {
    for i in 0..ANSI_FONTS {
        if let Ok(ansi_font) = BitFont::from_ansi_font_page(i) {
            if ansi_font == *font {
                return ansi_font.name().to_string();
            }
        }
    }

    for name in SAUCE_FONT_NAMES {
        if let Ok(sauce_font) = BitFont::from_sauce_name(name) {
            if sauce_font == *font {
                return sauce_font.name().to_string();
            }
        }
    }
    "Unknown".to_string()
    /*
    fl!(
        crate::LANGUAGE_LOADER,
        "unknown-font-name",
        width = font.size().width,
        height = font.size().height
    )*/
}
