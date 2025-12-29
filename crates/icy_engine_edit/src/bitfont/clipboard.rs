//! Clipboard functionality for BitFont editing
//!
//! This module provides clipboard operations for copying and pasting
//! pixel selections from bitmap font glyphs. Uses a custom clipboard
//! format that stores width, height, and pixel data.
//!
//! The clipboard operations return Tasks that need to be executed
//! by the iced runtime.

use iced::clipboard::STANDARD;
use iced::Task;

/// Custom clipboard type identifier for BitFont pixel data
pub const BITFONT_CLIPBOARD_TYPE: &str = "application/x-icy-bitfont";

/// Error type for clipboard operations
#[derive(Debug, Clone)]
pub enum BitFontClipboardError {
    /// No selection available to copy
    NoSelection,
    /// Failed to set clipboard contents
    ClipboardSetFailed(String),
    /// Failed to get clipboard contents
    ClipboardGetFailed(String),
    /// Invalid clipboard data format
    InvalidFormat,
    /// Clipboard doesn't contain BitFont data
    NoBitFontData,
}

impl std::fmt::Display for BitFontClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BitFontClipboardError::NoSelection => write!(f, "No selection available to copy"),
            BitFontClipboardError::ClipboardSetFailed(msg) => write!(f, "Failed to set clipboard: {}", msg),
            BitFontClipboardError::ClipboardGetFailed(msg) => write!(f, "Failed to get clipboard: {}", msg),
            BitFontClipboardError::InvalidFormat => write!(f, "Invalid clipboard data format"),
            BitFontClipboardError::NoBitFontData => write!(f, "Clipboard doesn't contain BitFont data"),
        }
    }
}

impl std::error::Error for BitFontClipboardError {}

/// Data structure for clipboard pixel data
#[derive(Debug, Clone)]
pub struct BitFontClipboardData {
    /// Width of the pixel region
    pub width: u8,
    /// Height of the pixel region
    pub height: u8,
    /// Pixel data: row-major, packed bits (8 pixels per byte)
    pub pixels: Vec<Vec<bool>>,
}

impl BitFontClipboardData {
    /// Create clipboard data from a pixel region
    pub fn new(pixels: Vec<Vec<bool>>) -> Self {
        let height = pixels.len() as u8;
        let width = if pixels.is_empty() { 0 } else { pixels[0].len() as u8 };
        Self { width, height, pixels }
    }

    /// Serialize to bytes: width (1 byte) + height (1 byte) + pixel data (packed bits)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(self.width);
        data.push(self.height);

        // Pack pixels into bytes (8 pixels per byte, row by row)
        for row in &self.pixels {
            let mut byte = 0u8;
            let mut bit_pos = 0;
            for &pixel in row {
                if pixel {
                    byte |= 1 << (7 - bit_pos);
                }
                bit_pos += 1;
                if bit_pos == 8 {
                    data.push(byte);
                    byte = 0;
                    bit_pos = 0;
                }
            }
            // Push remaining bits if row width is not multiple of 8
            if bit_pos > 0 {
                data.push(byte);
            }
        }

        data
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, BitFontClipboardError> {
        if data.len() < 2 {
            return Err(BitFontClipboardError::InvalidFormat);
        }

        let width = data[0];
        let height = data[1];

        if width == 0 || height == 0 {
            return Err(BitFontClipboardError::InvalidFormat);
        }

        // Calculate expected byte count per row
        let bytes_per_row = (width as usize + 7) / 8;
        let expected_data_len = 2 + bytes_per_row * height as usize;

        if data.len() < expected_data_len {
            return Err(BitFontClipboardError::InvalidFormat);
        }

        // Unpack pixels
        let mut pixels = Vec::with_capacity(height as usize);
        let mut byte_idx = 2;

        for _ in 0..height {
            let mut row = Vec::with_capacity(width as usize);
            let mut bit_pos = 0;
            let mut current_byte = data[byte_idx];

            for _ in 0..width {
                let pixel = (current_byte >> (7 - bit_pos)) & 1 == 1;
                row.push(pixel);
                bit_pos += 1;
                if bit_pos == 8 {
                    byte_idx += 1;
                    if byte_idx < data.len() {
                        current_byte = data[byte_idx];
                    }
                    bit_pos = 0;
                }
            }
            // Move to next row (skip remaining bits if row width is not multiple of 8)
            if bit_pos > 0 {
                byte_idx += 1;
            }
            pixels.push(row);
        }

        Ok(Self { width, height, pixels })
    }
}

/// Copy pixel data to clipboard (returns a Task to be executed by iced runtime)
pub fn copy_to_clipboard<Message: Send + 'static>(
    data: &BitFontClipboardData,
    on_complete: impl Fn(Result<(), BitFontClipboardError>) -> Message + Send + 'static,
) -> Task<Message> {
    let bytes = data.to_bytes();
    STANDARD.write_format(bytes, &[BITFONT_CLIPBOARD_TYPE]).map(move |()| on_complete(Ok(())))
}

/// Get pixel data from clipboard (returns a Task to be executed by iced runtime)
pub fn get_from_clipboard<Message: Send + 'static>(
    on_complete: impl Fn(Result<BitFontClipboardData, BitFontClipboardError>) -> Message + Send + 'static,
) -> Task<Message> {
    STANDARD.read_format(&[BITFONT_CLIPBOARD_TYPE]).map(move |result| {
        let parsed = match result {
            Some(data) => BitFontClipboardData::from_bytes(&data.data),
            None => Err(BitFontClipboardError::NoBitFontData),
        };
        on_complete(parsed)
    })
}

/// Check if clipboard has BitFont data (returns a Task to be executed by iced runtime)
pub fn has_bitfont_data<Message: Send + 'static>(on_result: impl Fn(bool) -> Message + Send + 'static) -> Task<Message> {
    STANDARD.has_format(vec![BITFONT_CLIPBOARD_TYPE.to_string()]).map(on_result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_data_roundtrip() {
        let pixels = vec![vec![true, false, true, false], vec![false, true, false, true], vec![true, true, true, true]];
        let data = BitFontClipboardData::new(pixels.clone());

        let bytes = data.to_bytes();
        let restored = BitFontClipboardData::from_bytes(&bytes).unwrap();

        assert_eq!(restored.width, 4);
        assert_eq!(restored.height, 3);
        assert_eq!(restored.pixels, pixels);
    }

    #[test]
    fn test_clipboard_data_8x8() {
        let mut pixels = Vec::new();
        for y in 0..8 {
            let mut row = Vec::new();
            for x in 0..8 {
                row.push((x + y) % 2 == 0);
            }
            pixels.push(row);
        }
        let data = BitFontClipboardData::new(pixels.clone());

        let bytes = data.to_bytes();
        assert_eq!(bytes.len(), 2 + 8); // 2 header bytes + 8 bytes for 8x8 data

        let restored = BitFontClipboardData::from_bytes(&bytes).unwrap();
        assert_eq!(restored.pixels, pixels);
    }

    #[test]
    fn test_clipboard_data_non_aligned_width() {
        // 5 pixels wide (not byte-aligned)
        let pixels = vec![vec![true, false, true, false, true], vec![false, true, false, true, false]];
        let data = BitFontClipboardData::new(pixels.clone());

        let bytes = data.to_bytes();
        let restored = BitFontClipboardData::from_bytes(&bytes).unwrap();

        assert_eq!(restored.width, 5);
        assert_eq!(restored.height, 2);
        assert_eq!(restored.pixels, pixels);
    }

    #[test]
    fn test_invalid_format() {
        // Too short
        assert!(BitFontClipboardData::from_bytes(&[]).is_err());
        assert!(BitFontClipboardData::from_bytes(&[8]).is_err());

        // Zero dimensions
        assert!(BitFontClipboardData::from_bytes(&[0, 8]).is_err());
        assert!(BitFontClipboardData::from_bytes(&[8, 0]).is_err());
    }
}
