use std::path::PathBuf;
use std::sync::Arc;

use icy_sauce::SauceRecord;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::create_text_preview;

use super::tile_shader::TILE_IMAGE_WIDTH;

// ============================================================================
// Thumbnail Rendering Constants
// ============================================================================

/// Thumbnails are rendered at 2x display size for quality, then scaled down
/// This is the render width (640px renders to 320px display)
pub const THUMBNAIL_RENDER_WIDTH: u32 = 640;

/// Maximum thumbnail height (to prevent memory issues with very tall images)
pub const THUMBNAIL_MAX_HEIGHT: u32 = 2000;

/// Scale factor from render size to display size
/// TILE_IMAGE_WIDTH (320) / THUMBNAIL_RENDER_WIDTH (640) = 0.5
pub const THUMBNAIL_SCALE: f32 = TILE_IMAGE_WIDTH / THUMBNAIL_RENDER_WIDTH as f32;

// ============================================================================
// Static Placeholder Images (shared across all tiles)
// ============================================================================

/// Static placeholder for loading tiles - shows a loading spinner symbol
pub static LOADING_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| create_text_preview("Loading..."));

/// Static placeholder for error tiles (128x96 with X symbol)
pub static ERROR_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| {
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
});

/// Static placeholder for folder tiles (128x96 with folder icon)
pub static FOLDER_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| {
    let width = 128u32;
    let height = 96u32;
    let mut data = vec![0u8; (width * height * 4) as usize];

    let folder_color: [u8; 4] = [180, 140, 60, 255];
    let tab_color: [u8; 4] = [160, 120, 40, 255];
    let outline_color: [u8; 4] = [100, 80, 30, 255];

    let body_top = 24;
    let body_left = 8;
    let body_right = width - 8;
    let body_bottom = height - 8;

    for y in body_top..body_bottom {
        for x in body_left..body_right {
            let idx = ((y * width + x) * 4) as usize;
            if x == body_left || x == body_right - 1 || y == body_top || y == body_bottom - 1 {
                data[idx..idx + 4].copy_from_slice(&outline_color);
            } else {
                data[idx..idx + 4].copy_from_slice(&folder_color);
            }
        }
    }

    let tab_top = 12;
    let tab_left = 8;
    let tab_right = 48;
    let tab_bottom = body_top + 4;

    for y in tab_top..tab_bottom {
        for x in tab_left..tab_right {
            let idx = ((y * width + x) * 4) as usize;
            if x == tab_left || x == tab_right - 1 || y == tab_top {
                data[idx..idx + 4].copy_from_slice(&outline_color);
            } else {
                data[idx..idx + 4].copy_from_slice(&tab_color);
            }
        }
    }

    RgbaData::new(data, width, height)
});

/// Static placeholder for FILE_ID.DIZ not found (128x96 with ? symbol)
pub static DIZ_NOT_FOUND_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| create_text_preview("no file_id.diz"));

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

/// RGBA image data for shader rendering
#[derive(Debug, Clone)]
pub struct RgbaData {
    /// Raw RGBA pixel data (Arc for cheap cloning)
    pub data: Arc<Vec<u8>>,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

impl RgbaData {
    pub fn new(data: Vec<u8>, width: u32, height: u32) -> Self {
        let expected_size = (4 * width * height) as usize;
        let actual_size = data.len();

        // If data size doesn't match, create a properly sized buffer
        let valid_data = if actual_size != expected_size {
            log::warn!(
                "RgbaData size mismatch: expected {} bytes ({}x{}x4), got {} bytes. Padding/truncating.",
                expected_size,
                width,
                height,
                actual_size
            );
            let mut fixed = vec![0u8; expected_size];
            let copy_size = actual_size.min(expected_size);
            fixed[..copy_size].copy_from_slice(&data[..copy_size]);
            fixed
        } else {
            data
        };

        Self {
            data: Arc::new(valid_data),
            width,
            height,
        }
    }
}

impl PartialEq for RgbaData {
    fn eq(&self, other: &Self) -> bool {
        // Compare by Arc pointer for efficiency (same Arc = same data)
        Arc::ptr_eq(&self.data, &other.data) && self.width == other.width && self.height == other.height
    }
}

impl Eq for RgbaData {}

impl std::hash::Hash for RgbaData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash by Arc pointer for efficiency
        Arc::as_ptr(&self.data).hash(state);
        self.width.hash(state);
        self.height.hash(state);
    }
}

/// State of a thumbnail
#[derive(Debug, Clone)]
pub enum ThumbnailState {
    /// Not yet loaded
    Pending,
    /// Currently being loaded
    Loading,
    /// Failed to load
    Error(String),
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
    /// Get the current RGBA data to display
    pub fn current_rgba(&self) -> Option<&RgbaData> {
        match self {
            ThumbnailState::Ready { rgba } => Some(rgba),
            ThumbnailState::Animated { frames, current_frame } => frames.get(*current_frame),
            _ => None,
        }
    }

    /// Get the dimensions of the thumbnail
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        match self {
            ThumbnailState::Ready { rgba } => Some((rgba.width, rgba.height)),
            ThumbnailState::Animated { frames, .. } => frames.first().map(|f| (f.width, f.height)),
            _ => None,
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

    /// Check if this thumbnail is ready (loaded successfully)
    pub fn is_ready(&self) -> bool {
        matches!(self, ThumbnailState::Ready { .. } | ThumbnailState::Animated { .. })
    }
}

/// A thumbnail entry in the cache
#[derive(Clone)]
pub struct Thumbnail {
    /// Path to the source file
    pub path: PathBuf,
    /// Display label (filename)
    pub label: String,
    /// Current state
    pub state: ThumbnailState,
    /// SAUCE information (if available)
    pub sauce_info: Option<SauceRecord>,
    /// Width multiplier (1, 2, or 3 based on character columns)
    pub width_multiplier: u32,
    /// Rendered label tag (DOS-style with IBM font)
    pub label_tag: Option<RgbaData>,
}

impl Thumbnail {
    /// Create a new pending thumbnail
    pub fn new(path: PathBuf, label: String) -> Self {
        Self {
            path,
            label,
            state: ThumbnailState::Pending,
            sauce_info: None,
            width_multiplier: 1,
            label_tag: None,
        }
    }

    /// Get the width multiplier for this thumbnail (1, 2, or 3)
    pub fn get_width_multiplier(&self) -> u32 {
        self.width_multiplier.max(1).min(3)
    }

    /// Get the display height for layout purposes
    /// The thumbnail is rendered at THUMBNAIL_RENDER_WIDTH * width_multiplier pixels
    /// and scaled down by THUMBNAIL_SCALE for display
    pub fn display_height(&self, display_width: f32) -> f32 {
        match self.state.dimensions() {
            Some((w, h)) => {
                if w == 0 {
                    return 100.0; // Fallback for invalid dimensions
                }
                // Scale height proportionally: display_width / render_width * render_height
                (display_width / w as f32) * h as f32
            }
            None => 100.0, // Default height for pending/loading
        }
    }

    /// Get the display width for layout purposes (in units of base width)
    pub fn display_width(&self, base_width: f32, base_scale: f32) -> f32 {
        base_width * base_scale * self.get_width_multiplier() as f32
    }
}

/// Result from the thumbnail loader thread
pub struct ThumbnailResult {
    /// Path of the loaded file
    pub path: PathBuf,
    /// Loaded state
    pub state: ThumbnailState,
    /// SAUCE information (if available)
    pub sauce_info: Option<SauceRecord>,
    /// Width multiplier (1, 2, or 3 based on character columns)
    pub width_multiplier: u32,
    /// Rendered label tag (DOS-style with IBM font)
    pub label_tag: Option<RgbaData>,
}

/// Shared thumbnail cache
pub type ThumbnailCache = Arc<Mutex<Vec<Thumbnail>>>;
