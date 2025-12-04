use std::path::PathBuf;
use std::sync::Arc;

use i18n_embed_fl::fl;
use icy_sauce::SauceRecord;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use super::thumbnail_loader::create_labeled_placeholder;
use crate::{LANGUAGE_LOADER, create_text_preview};

// ============================================================================
// Thumbnail Rendering Constants
// ============================================================================

/// Thumbnails are rendered directly at display size (no scaling needed)
/// This matches TILE_IMAGE_WIDTH (320px)
pub const THUMBNAIL_RENDER_WIDTH: u32 = 320;

/// Maximum thumbnail height - limited by GPU texture size
/// GPU max is 8192, we use 8000 to have some margin
pub const THUMBNAIL_MAX_HEIGHT: u32 = 8000;

/// Scale factor from render size to display size
/// Now 1.0 since we render at final size
pub const THUMBNAIL_SCALE: f32 = 1.0;

/// Scale a small placeholder image to THUMBNAIL_RENDER_WIDTH, maintaining aspect ratio
fn scale_placeholder(rgba: RgbaData) -> RgbaData {
    if rgba.width == 0 || rgba.height == 0 {
        return rgba;
    }

    let target_width = THUMBNAIL_RENDER_WIDTH;
    let scale = target_width as f32 / rgba.width as f32;
    let target_height = ((rgba.height as f32 * scale) as u32).max(1);

    if target_width == rgba.width && target_height == rgba.height {
        return rgba;
    }

    // Use image crate to scale
    let img = image::RgbaImage::from_raw(rgba.width, rgba.height, rgba.data.to_vec()).unwrap_or_else(|| image::RgbaImage::new(1, 1));
    let scaled = image::imageops::resize(&img, target_width, target_height, image::imageops::FilterType::Triangle);

    RgbaData::new(scaled.into_raw(), target_width, target_height)
}

// ============================================================================
// Static Placeholder Images (shared across all tiles)
// All placeholders are pre-scaled to THUMBNAIL_RENDER_WIDTH (320px)
// ============================================================================

/// Static placeholder for loading tiles - shows translated "Loading..."
pub static LOADING_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| {
    let text = fl!(LANGUAGE_LOADER, "thumbnail-loading");
    let base = create_text_preview(&text);
    scale_placeholder(base)
});

/// Static placeholder for error tiles (scaled to 320px, with X symbol)
pub static ERROR_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| {
    let base = create_error_placeholder_base();
    scale_placeholder(base)
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

/// Static placeholder for folder tiles (scaled to 320px, with folder icon)
pub static FOLDER_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| {
    let base = create_folder_placeholder_base();
    scale_placeholder(base)
});

fn create_folder_placeholder_base() -> RgbaData {
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
}

/// Static placeholder for FILE_ID.DIZ not found - pre-scaled to 320px
/// Note: Label is added later by the thumbnail loader using the actual item name
pub static DIZ_NOT_FOUND_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| {
    let text = fl!(LANGUAGE_LOADER, "thumbnail-no-diz");
    let base = create_text_preview(&text);
    scale_placeholder(base)
});

/// Static placeholder for unsupported file formats - pre-scaled to 320px
/// Note: Label is added later by the thumbnail loader using the actual item name
pub static UNSUPPORTED_PLACEHOLDER: Lazy<RgbaData> = Lazy::new(|| {
    let text = fl!(LANGUAGE_LOADER, "thumbnail-unsupported");
    let base = create_text_preview(&text);
    scale_placeholder(base)
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
    /// Get the current RGBA data to display
    pub fn current_rgba(&self) -> Option<&RgbaData> {
        match self {
            ThumbnailState::Ready { rgba } => Some(rgba),
            ThumbnailState::Animated { frames, current_frame } => frames.get(*current_frame),
            ThumbnailState::Pending { placeholder } => placeholder.as_ref(),
            ThumbnailState::Loading { placeholder } => placeholder.as_ref(),
            ThumbnailState::Error { placeholder, .. } => placeholder.as_ref(),
        }
    }

    /// Get the dimensions of the thumbnail
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        match self {
            ThumbnailState::Ready { rgba } => Some((rgba.width, rgba.height)),
            ThumbnailState::Animated { frames, .. } => frames.first().map(|f| (f.width, f.height)),
            ThumbnailState::Pending { placeholder } | ThumbnailState::Loading { placeholder } => placeholder.as_ref().map(|p| (p.width, p.height)),
            ThumbnailState::Error { placeholder, .. } => placeholder.as_ref().map(|p| (p.width, p.height)),
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
    pub path: PathBuf,
    /// Display label (filename)
    pub label: String,
    /// Current state
    pub state: ThumbnailState,
    /// SAUCE information (if available)
    pub sauce_info: Option<SauceRecord>,
    /// Width multiplier (1, 2, or 3 based on character columns)
    pub width_multiplier: u32,
}

impl Thumbnail {
    /// Create a new pending thumbnail with pre-rendered placeholder including label
    pub fn new(path: PathBuf, label: String) -> Self {
        // Create placeholder with label for immediate display
        let placeholder = create_labeled_placeholder(&LOADING_PLACEHOLDER, &label);
        Self {
            path,
            label,
            state: ThumbnailState::Pending {
                placeholder: Some(placeholder),
            },
            sauce_info: None,
            width_multiplier: 1,
        }
    }

    /// Get the width multiplier for this thumbnail (1, 2, or 3)
    pub fn get_width_multiplier(&self) -> u32 {
        self.width_multiplier.max(1).min(3)
    }

    /// Set state to Loading, keeping or creating the placeholder with label
    pub fn set_loading(&mut self) {
        let placeholder = match &self.state {
            ThumbnailState::Pending { placeholder } => placeholder.clone(),
            _ => Some(create_labeled_placeholder(&LOADING_PLACEHOLDER, &self.label)),
        };
        self.state = ThumbnailState::Loading { placeholder };
    }

    /// Set state to Error with message and placeholder
    pub fn set_error(&mut self, message: String) {
        let placeholder = create_labeled_placeholder(&ERROR_PLACEHOLDER, &self.label);
        self.state = ThumbnailState::Error {
            message,
            placeholder: Some(placeholder),
        };
    }

    /// Get the display height for layout purposes
    /// content_width is the width available for the image (tile width minus padding)
    /// Returns the height the image would have when displayed at content_width
    ///
    /// The texture is rendered at final size, so no scaling is needed.
    pub fn display_height(&self, content_width: f32) -> f32 {
        match self.state.dimensions() {
            Some((tex_w, tex_h)) => {
                if tex_w == 0 {
                    return 100.0; // Fallback for invalid dimensions
                }
                // Texture is at final display size, scale to fit content_width
                let scale = content_width / tex_w as f32;
                tex_h as f32 * scale
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
}

/// Shared thumbnail cache
pub type ThumbnailCache = Arc<Mutex<Vec<Thumbnail>>>;
