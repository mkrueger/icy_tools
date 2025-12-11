//! Shared clipboard functionality for ICY applications
//!
//! This module provides a unified clipboard implementation for copying
//! selections from terminal/screen buffers. It handles all clipboard formats:
//! - Plain text
//! - RTF (rich text with colors and attributes)
//! - Image (rendered selection)
//! - ICY binary format (for paste between ICY applications)
//!
//! The implementation handles OS-specific quirks (e.g., Windows requires
//! text to be last in the clipboard contents).

use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContent, RustImageData};
use icy_engine::{RenderOptions, Screen};
use image::DynamicImage;

/// Clipboard type identifier for ICY binary format
pub const ICY_CLIPBOARD_TYPE: &str = "application/x-icy-buffer";

/// Error type for clipboard operations
#[derive(Debug)]
pub enum ClipboardError {
    /// No selection available to copy
    NoSelection,
    /// Failed to create image from rendered data
    ImageCreationFailed,
    /// Failed to set clipboard contents
    ClipboardSetFailed(String),
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardError::NoSelection => write!(f, "No selection available to copy"),
            ClipboardError::ImageCreationFailed => write!(f, "Failed to create image from rendered data"),
            ClipboardError::ClipboardSetFailed(msg) => write!(f, "Failed to set clipboard: {}", msg),
        }
    }
}

impl std::error::Error for ClipboardError {}

/// Copy the current selection to clipboard with all formats (text, RTF, image, ICY)
///
/// This function copies the selection in multiple formats to maximize compatibility:
/// - ICY binary format: For paste between ICY applications (preserves all attributes)
/// - Image: Rendered selection as RGBA image
/// - RTF: Rich text with colors and formatting
/// - Plain text: Simple text content
///
/// # Arguments
/// * `screen` - The screen/buffer containing the selection
/// * `clipboard` - The clipboard context to write to
/// * `options` - Rendering options for the image copy (9px font, aspect ratio)
///
/// # Returns
/// * `Ok(())` - Selection was copied successfully
/// * `Err(ClipboardError::NoSelection)` - No text available to copy
/// * `Err(ClipboardError::ClipboardSetFailed)` - Failed to write to clipboard
///
/// # Platform Notes
/// On Windows, the order of clipboard contents matters - text must be last
/// to be properly recognized by other applications.
pub fn copy_selection_to_clipboard<C: Clipboard>(screen: &mut dyn Screen, clipboard: &C) -> Result<(), ClipboardError> {
    // Get plain text first - if no text, nothing to copy
    let text = match screen.copy_text() {
        Some(t) => t,
        None => return Err(ClipboardError::NoSelection),
    };

    let mut contents = Vec::with_capacity(4);

    // ICY binary format (for paste between ICY applications)
    // This preserves all attributes, fonts, colors, etc.
    if let Some(data) = screen.clipboard_data() {
        contents.push(ClipboardContent::Other(ICY_CLIPBOARD_TYPE.into(), data));
    }

    // Image (rendered selection as RGBA)
    if let Some(selection) = screen.selection() {
        let (size, data) = screen.render_to_rgba(&RenderOptions {
            rect: selection,
            blink_on: true,
            selection: None,
            selection_fg: None,
            selection_bg: None,
            override_scan_lines: None,
        });

        if size.width > 0 && size.height > 0 {
            if let Some(img_buf) = image::ImageBuffer::from_raw(size.width as u32, size.height as u32, data) {
                let dynamic_image = DynamicImage::ImageRgba8(img_buf);
                let img = RustImageData::from_dynamic_image(dynamic_image);
                contents.push(ClipboardContent::Image(img));
            }
        }
    }

    // RTF (rich text with colors and formatting)
    if let Some(rich_text) = screen.copy_rich_text() {
        contents.push(ClipboardContent::Rtf(rich_text));
    }

    // Plain text - MUST be last on Windows to be recognized properly
    contents.push(ClipboardContent::Text(text));

    // Set all contents to clipboard
    clipboard.set(contents).map_err(|e| ClipboardError::ClipboardSetFailed(e.to_string()))?;

    // Clear selection after successful copy
    let _ = screen.clear_selection();

    Ok(())
}
