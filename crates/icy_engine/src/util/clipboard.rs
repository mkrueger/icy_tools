use std::error::Error;

use crate::{EngineResult, Position, Sixel};
use arboard::{Clipboard, ImageData};

pub const BUFFER_DATA: u16 = 0x0000;
pub const BITFONT_GLYPH: u16 = 0x0100;

/// .
///
/// # Errors
///
/// This function will return an error if .
pub fn push_data(clipboard: &mut arboard::Clipboard, data_type: u16, data: &[u8], _text: Option<String>) -> EngineResult<()> {
    let mut clipboard_data: Vec<u8> = Vec::new();
    clipboard_data.extend(b"iced");
    clipboard_data.extend(u16::to_le_bytes(data_type));
    clipboard_data.extend(data);
    while clipboard_data.len() % 4 != 0 {
        clipboard_data.push(0);
    }

    let image = ImageData {
        width: clipboard_data.len() / 4,
        height: 1,
        bytes: clipboard_data.into(),
    };
    clipboard.clear()?;
    if let Err(err) = clipboard.set_image(image) {
        return Err(ClipboardError::ErrorInSetImage(format!("{err}")).into());
    }
    Ok(())
}

pub fn pop_cliboard_text() -> Option<String> {
    match Clipboard::new() {
        Ok(mut clipboard) => {
            if let Ok(text) = clipboard.get_text() {
                return Some(text);
            }
        }
        Err(_) => {}
    }
    None
}

pub fn pop_data(data_type: u16) -> Option<Vec<u8>> {
    match Clipboard::new() {
        Ok(mut clipboard) => {
            if let Ok(img) = clipboard.get_image() {
                let data = img.bytes;
                if &data[0..4] == b"iced" && (data[4] as u16 | (data[5] as u16) << 8) == data_type {
                    let mut result = Vec::new();
                    result.extend(&data[6..]);
                    return Some(result);
                }
            }
        }
        Err(err) => {
            log::error!("Error creating clipboard: {err}");
        }
    }
    None
}

pub fn pop_sixel_image() -> Option<Sixel> {
    match Clipboard::new() {
        Ok(mut clipboard) => {
            if let Ok(img) = clipboard.get_image() {
                let mut sixel = Sixel::new(Position::default());
                sixel.picture_data = img.bytes.to_vec();
                sixel.set_width(img.width as i32);
                sixel.set_height(img.height as i32);
                return Some(sixel);
            }
        }
        Err(err) => {
            log::error!("Error creating clipboard: {err}");
        }
    }
    None
}

#[derive(Debug, Clone)]
enum ClipboardError {
    ErrorInSetImage(String),
    Error(String),
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardError::ErrorInSetImage(err) => {
                write!(f, "Error in setting image to clipboard: {err}")
            }
            ClipboardError::Error(err) => write!(f, "Error creating clipboard: {err}"),
        }
    }
}

impl Error for ClipboardError {
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
