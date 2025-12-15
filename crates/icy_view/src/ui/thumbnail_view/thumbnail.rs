use i18n_embed_fl::fl;
use icy_sauce::SauceRecord;
use once_cell::sync::Lazy;

use crate::LANGUAGE_LOADER;
use crate::items::create_text_preview;
use crate::thumbnail::scale_to_thumbnail_width;
pub use crate::thumbnail::{RgbaData, THUMBNAIL_MAX_HEIGHT, THUMBNAIL_RENDER_WIDTH};

// ============================================================================
// Thumbnail Rendering Constants
// ============================================================================

// ============================================================================
// Static Placeholder Images (shared across all tiles)
// All placeholders are pre-scaled to THUMBNAIL_RENDER_WIDTH (320px)
// ============================================================================

/// Static placeholder for loading tiles - shows translated "Loading..."
pub static LOADING_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| {
    let text = fl!(LANGUAGE_LOADER, "thumbnail-loading");
    let base = create_text_preview(&text);
    scale_to_thumbnail_width(base)
});

/// Static placeholder for error tiles (scaled to 320px, with X symbol)
pub static ERROR_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| {
    let base = create_error_placeholder_base();
    scale_to_thumbnail_width(base)
});

fn create_error_placeholder_base() -> RgbaData {
    let w = 128u32;
    let h = 96u32;
    let mut data = vec![0u8; (w * h * 4) as usize];

    let bg_color: [u8; 4] = [60, 20, 20, 255];
    let x_color: [u8; 4] = [200, 60, 60, 255];
    let border_color: [u8; 4] = [100, 40, 40, 255];

    for i in 0..(w * h) as usize {
        data[i * 4..i * 4 + 4].copy_from_slice(&bg_color);
    }

    for y in 0..h {
        for x in 0..w {
            if x < 2 || x >= w - 2 || y < 2 || y >= h - 2 {
                let idx = ((y * w + x) * 4) as usize;
                data[idx..idx + 4].copy_from_slice(&border_color);
            }
        }
    }

    let margin = 20u32;
    let thickness = 6i32;

    for y in margin..(h - margin) {
        for x in margin..(w - margin) {
            let norm_x = (x - margin) as f32 / (w - 2 * margin) as f32;
            let norm_y = (y - margin) as f32 / (h - 2 * margin) as f32;

            let on_diag1 = (norm_x - norm_y).abs() < (thickness as f32 / (w - 2 * margin) as f32);
            let on_diag2 = (norm_x - (1.0 - norm_y)).abs() < (thickness as f32 / (w - 2 * margin) as f32);

            if on_diag1 || on_diag2 {
                let idx = ((y * w + x) * 4) as usize;
                data[idx..idx + 4].copy_from_slice(&x_color);
            }
        }
    }

    RgbaData::new(data, w, h)
}

/// Static placeholder for unsupported file formats - pre-scaled to 320px
/// Note: Label is added later by the thumbnail loader using the actual item name
pub static UNSUPPORTED_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| {
    let text = fl!(LANGUAGE_LOADER, "thumbnail-unsupported");
    let base = create_text_preview(&text);
    scale_to_thumbnail_width(base)
});

/// Calculate the width multiplier based on character columns
/// 0-159 chars = 1x, 160-239 chars = 2x, 240+ chars = 3x
pub fn get_width_multiplier(char_columns: i32) -> u32 {
    if char_columns < 160 {
        1
    } else if char_columns < 240 {
        2
    } else {
        3
    }
}

// RgbaData and thumbnail constants live in crate::thumbnail (re-exported above)

/// State of a thumbnail
#[derive(Debug, Clone)]
pub enum ThumbnailState {
    /// Not yet loaded - shows loading placeholder with label
    Pending {
        /// Cached placeholder with label (created on first access)
        placeholder: Option<RgbaData>,
    },
    /// Currently being loaded - shows loading placeholder with label
    Loading {
        /// Cached placeholder with label
        placeholder: Option<RgbaData>,
    },
    /// Failed to load - shows error placeholder with label
    Error {
        /// Error message
        message: String,
        /// Cached placeholder with label
        placeholder: Option<RgbaData>,
    },
    /// Successfully loaded - static image
    Ready {
        /// The thumbnail image data
        rgba: RgbaData,
    },
    /// Successfully loaded - animated (blinking or GIF frames)
    Animated {
        /// Frame RGBA data (at least 2 for blinking)
        frames: Vec<RgbaData>,
        /// Current frame index
        current_frame: usize,
    },
}

impl ThumbnailState {
    /// Get the dimensions of the thumbnail
    /// Falls back to LOADING_PLACEHOLDER dimensions for pending/loading without placeholder
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        match self {
            ThumbnailState::Ready { rgba } => Some((rgba.width, rgba.height)),
            ThumbnailState::Animated { frames, .. } => frames.first().map(|f| (f.width, f.height)),
            ThumbnailState::Pending { placeholder } | ThumbnailState::Loading { placeholder } => placeholder
                .as_ref()
                .map(|p| (p.width, p.height))
                .or_else(|| Some((LOADING_PLACEHOLDER.width, LOADING_PLACEHOLDER.height))),
            ThumbnailState::Error { placeholder, .. } => placeholder
                .as_ref()
                .map(|p| (p.width, p.height))
                .or_else(|| Some((ERROR_PLACEHOLDER.width, ERROR_PLACEHOLDER.height))),
        }
    }

    /// Advance to next frame (for animated thumbnails)
    pub fn next_frame(&mut self) {
        if let ThumbnailState::Animated { frames, current_frame } = self {
            if !frames.is_empty() {
                *current_frame = (*current_frame + 1) % frames.len();
            }
        }
    }

    /// Check if this thumbnail is animated
    pub fn is_animated(&self) -> bool {
        matches!(self, ThumbnailState::Animated { .. })
    }

    /// Get the placeholder if available
    pub fn placeholder(&self) -> Option<&RgbaData> {
        match self {
            ThumbnailState::Pending { placeholder } => placeholder.as_ref(),
            ThumbnailState::Loading { placeholder } => placeholder.as_ref(),
            ThumbnailState::Error { placeholder, .. } => placeholder.as_ref(),
            _ => None,
        }
    }
}

/// A thumbnail entry in the cache
#[derive(Clone)]
pub struct Thumbnail {
    /// Path to the source file
    pub path: String,
    /// Display label (filename)
    pub label: String,
    /// Current state
    pub state: ThumbnailState,
    /// SAUCE information (if available)
    pub sauce_info: Option<SauceRecord>,
    /// Width multiplier (1, 2, or 3 based on character columns)
    pub width_multiplier: u32,
    /// Label RGBA data (rendered separately for GPU)
    pub label_rgba: Option<RgbaData>,
}

impl Thumbnail {
    /// Create a new pending thumbnail
    /// Placeholder is created lazily when the thumbnail is first displayed
    /// This is critical for performance with 100k+ items
    pub fn new(path: String, label: String) -> Self {
        Self {
            path,
            label,
            state: ThumbnailState::Pending {
                placeholder: None, // Lazy - created when needed
            },
            sauce_info: None,
            width_multiplier: 1,
            label_rgba: None,
        }
    }

    /// Get the width multiplier for this thumbnail (1, 2, or 3)
    pub fn get_width_multiplier(&self) -> u32 {
        self.width_multiplier.max(1).min(3)
    }

    /// Get the display height for layout purposes
    /// content_width is the width available for the image (tile width minus padding)
    /// Returns the height the image would have when displayed at content_width
    /// Now includes label height since labels are rendered separately
    ///
    /// The raw texture dimensions are scaled to fit content_width.
    pub fn display_height(&self, content_width: f32) -> f32 {
        let image_height = match self.state.dimensions() {
            Some((tex_w, tex_h)) => {
                if tex_w == 0 {
                    100.0 // Fallback for invalid dimensions
                } else {
                    // Scale raw texture to fit content_width
                    let scale = content_width / tex_w as f32;
                    tex_h as f32 * scale
                }
            }
            None => 100.0, // Default height for pending/loading
        };

        // Label is rendered at 2x scale for readability
        const LABEL_SCALE: f32 = 2.0;
        let label_height = self.label_rgba.as_ref().map(|l| l.height as f32 * LABEL_SCALE).unwrap_or(0.0);

        // Add separator space between image and label (matching TILE_INNER_PADDING)
        let separator = if label_height > 0.0 { 4.0 } else { 0.0 };

        image_height + separator + label_height
    }
}

/// Result from the thumbnail loader thread
pub struct ThumbnailResult {
    /// Path of the loaded file
    pub path: String,
    /// Loaded state
    pub state: ThumbnailState,
    /// SAUCE information (if available)
    pub sauce_info: Option<SauceRecord>,
    /// Width multiplier (1, 2, or 3 based on character columns)
    pub width_multiplier: u32,
    /// Label RGBA data (rendered separately for GPU)
    pub label_rgba: Option<RgbaData>,
}
