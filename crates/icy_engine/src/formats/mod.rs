pub(crate) mod io;

mod bitfont_format;
pub use bitfont_format::*;

mod character_font_format;
pub use character_font_format::*;

mod file_format;
pub use file_format::*;

pub mod ansi_v2;
pub use ansi_v2::{AnsiCompatibilityLevel, SaveOptions, SixelSettings, save_ansi_v2};

mod image_format;
use icy_sauce::SauceRecord;
pub use image_format::*;

mod palette_format;
pub use palette_format::*;

use serde::{Deserialize, Serialize};

mod color_optimization;
pub use color_optimization::*;

pub use io::seq::seq_prepare;

use crate::{ANSI_FONTS, BitFont, EditableScreen, Layer, Result, Role, Screen, Size, TextPane, TextScreen, get_sauce_font_names};
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

/// Options for loading files.
///
/// Note: SAUCE metadata is returned separately in `LoadedDocument`, not passed as input.
#[derive(Default)]
pub struct LoadData {
    ansi_music: Option<MusicOption>,
    default_terminal_width: Option<usize>,
    /// Optional maximum height limit for the buffer
    /// If set, the buffer height will be clamped to this value after loading
    max_height: Option<i32>,
    convert_to_utf8: bool,
}

impl LoadData {
    pub fn new(ansi_music: Option<MusicOption>, default_terminal_width: Option<usize>) -> Self {
        LoadData {
            ansi_music,
            default_terminal_width,
            max_height: None,
            convert_to_utf8: false,
        }
    }

    pub fn ansi_music(&self) -> Option<MusicOption> {
        self.ansi_music
    }

    pub fn default_terminal_width(&self) -> Option<usize> {
        self.default_terminal_width
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

/// Result of loading a document.
///
/// Contains both the loaded screen and any SAUCE metadata found in the file.
/// SAUCE settings (size, font, ice colors) are automatically applied to the screen,
/// but the original SAUCE record is also available for access to title, author, group, etc.
pub struct LoadedDocument {
    /// The loaded screen with SAUCE settings applied
    pub screen: TextScreen,
    /// The original SAUCE record, if present in the file
    pub sauce_opt: Option<icy_sauce::SauceRecord>,
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
                let size = buf.size();
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

/// Parse data using a CommandParser from icy_parser_core
///
/// # Errors
///
/// Returns an error if sixel processing fails
pub fn load_with_parser(result: &mut TextScreen, interpreter: &mut dyn CommandParser, data: &[u8], _skip_errors: bool, min_height: i32) -> Result<()> {
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
                let size = sixel.size();
                let font_size = result.buffer.font_dimensions();
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
        let real_height = result.buffer.line_count().max(result.caret.y + 1).max(min_height);
        result.buffer.set_height(real_height);
    }

    let height = result.height();
    let width = result.width();
    for y in 0..height {
        for x in 0..width {
            let mut ch = result.char_at((x, y).into());
            if ch.attribute.is_bold() {
                let fg = ch.attribute.foreground();
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
    for i in 0..ANSI_FONTS as u8 {
        if let Some(ansi_font) = BitFont::from_ansi_font_page(i, 16) {
            if *ansi_font == *font {
                return ansi_font.name().to_string();
            }
        }
    }

    for name in get_sauce_font_names() {
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
