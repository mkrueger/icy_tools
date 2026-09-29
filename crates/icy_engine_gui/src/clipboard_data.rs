//! The formats a screen selection is copied in, shared by the iced and egui frontends:
//! - Plain text
//! - RTF (rich text with colors and attributes)
//! - Image (rendered selection)
//! - ICY binary format (for paste between ICY applications)

use icy_engine::{RenderOptions, Screen};

/// Clipboard type identifier for ICY binary format
pub use icy_engine::clipboard::ICY_CLIPBOARD_TYPE;

/// Error type for clipboard operations
#[derive(Debug, Clone)]
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

/// Prepared clipboard data ready to be written
/// This struct holds all the data that will be written to the clipboard
#[derive(Debug, Clone)]
pub struct ClipboardData {
    /// Plain text content
    pub text: String,
    /// RTF content (optional)
    pub rtf: Option<String>,
    /// Image data as RGBA bytes with dimensions (optional)
    pub image: Option<(Vec<u8>, u32, u32)>,
    /// ICY binary format data (optional)
    pub icy_data: Option<Vec<u8>>,
}

/// Prepare clipboard data from a screen selection
///
/// This function extracts all clipboard formats from the selection:
/// - ICY binary format: For paste between ICY applications (preserves all attributes)
/// - Image: Rendered selection as RGBA image
/// - RTF: Rich text with colors and formatting
/// - Plain text: Simple text content
///
/// # Arguments
/// * `screen` - The screen/buffer containing the selection
///
/// # Returns
/// * `Ok(ClipboardData)` - Data ready to be written to clipboard
/// * `Err(ClipboardError::NoSelection)` - No text available to copy
pub fn prepare_clipboard_data(screen: &dyn Screen) -> Result<ClipboardData, ClipboardError> {
    // Get plain text first - if no text, nothing to copy
    let text = match screen.copy_text() {
        Some(t) => t,
        None => return Err(ClipboardError::NoSelection),
    };

    // ICY binary format (for paste between ICY applications)
    let icy_data = screen.clipboard_data();

    // Image (rendered selection as RGBA)
    let image = if let Some(selection) = screen.selection() {
        let (mut size, mut data) = screen.render_to_rgba_raw(&RenderOptions {
            rect: selection,
            blink_on: true,
            selection: None,
            selection_fg: None,
            selection_bg: None,
            override_scan_lines: None,
        });

        // Aspect ratio correction is applied at display/shader level.
        // For clipboard images, apply the same stretch so it matches what the user sees.
        if screen.use_aspect_ratio() {
            let scale = screen.aspect_ratio_stretch_factor();
            let (scaled_h, scaled_pixels) = scale_image_vertical(data, size.width, size.height, scale);
            size.height = scaled_h;
            data = scaled_pixels;
        }

        if size.width > 0 && size.height > 0 {
            Some((data, size.width as u32, size.height as u32))
        } else {
            None
        }
    } else {
        None
    };

    // RTF (rich text with colors and formatting)
    let rtf = screen.copy_rich_text();

    Ok(ClipboardData { text, rtf, image, icy_data })
}

fn scale_image_vertical(pixels: Vec<u8>, width: i32, height: i32, scale: f32) -> (i32, Vec<u8>) {
    let new_height = (height as f32 * scale).round() as i32;
    if new_height <= 0 || width <= 0 || height <= 0 || scale <= 0.0 {
        return (height, pixels);
    }

    let stride = width as usize * 4;
    let mut scaled = vec![0u8; stride * new_height as usize];

    for new_y in 0..new_height {
        let src_y = new_y as f32 / scale;
        let src_y0 = (src_y.floor() as i32).clamp(0, height - 1) as usize;
        let src_y1 = (src_y0 + 1).min(height as usize - 1);
        let t = src_y.fract();

        let dst_row = new_y as usize * stride;
        let src_row0 = src_y0 * stride;
        let src_row1 = src_y1 * stride;

        for x in 0..width as usize {
            let px = x * 4;

            let a0 = pixels[src_row0 + px + 3] as f32;
            let a1 = pixels[src_row1 + px + 3] as f32;
            let a = a0 + (a1 - a0) * t;
            let out_a = if a >= 128.0 { 255u8 } else { 0u8 };

            if out_a == 0 {
                scaled[dst_row + px] = 0;
                scaled[dst_row + px + 1] = 0;
                scaled[dst_row + px + 2] = 0;
                scaled[dst_row + px + 3] = 0;
                continue;
            }

            let mut w0 = 1.0 - t;
            let mut w1 = t;
            if pixels[src_row0 + px + 3] == 0 {
                w0 = 0.0;
            }
            if pixels[src_row1 + px + 3] == 0 {
                w1 = 0.0;
            }

            let w_sum = w0 + w1;
            if w_sum <= f32::EPSILON {
                scaled[dst_row + px] = 0;
                scaled[dst_row + px + 1] = 0;
                scaled[dst_row + px + 2] = 0;
                scaled[dst_row + px + 3] = 0;
                continue;
            }
            w0 /= w_sum;
            w1 /= w_sum;

            for c in 0..3 {
                let v0 = pixels[src_row0 + px + c] as f32;
                let v1 = pixels[src_row1 + px + c] as f32;
                scaled[dst_row + px + c] = (v0 * w0 + v1 * w1).round() as u8;
            }
            scaled[dst_row + px + 3] = 255;
        }
    }

    (new_height, scaled)
}
