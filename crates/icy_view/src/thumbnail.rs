use std::sync::Arc;

/// Thumbnails are rendered directly at display size (no scaling needed)
/// This matches TILE_IMAGE_WIDTH (320px)
pub const THUMBNAIL_RENDER_WIDTH: u32 = 320;

/// Maximum thumbnail height - limited to 80000px (10 slices × 8000px per slice)
/// Very tall images beyond this are cropped to show the top portion
pub const THUMBNAIL_MAX_HEIGHT: u32 = 80000;

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

/// Scale a small image to `THUMBNAIL_RENDER_WIDTH`, maintaining aspect ratio.
pub fn scale_to_thumbnail_width(rgba: RgbaData) -> RgbaData {
    if rgba.width == 0 || rgba.height == 0 {
        return rgba;
    }

    let target_width = THUMBNAIL_RENDER_WIDTH;
    let scale = target_width as f32 / rgba.width as f32;
    let target_height = ((rgba.height as f32 * scale) as u32).max(1);

    if target_width == rgba.width && target_height == rgba.height {
        return rgba;
    }

    let img = image::RgbaImage::from_raw(rgba.width, rgba.height, rgba.data.to_vec()).unwrap_or_else(|| image::RgbaImage::new(1, 1));
    let scaled = image::imageops::resize(&img, target_width, target_height, image::imageops::FilterType::Triangle);

    RgbaData::new(scaled.into_raw(), target_width, target_height)
}
